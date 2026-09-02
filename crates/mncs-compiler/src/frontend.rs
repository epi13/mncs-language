use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use mncs_model::{
    binding_id, binding_id_for, finite_type_id, finite_variant_id, function_id, module_id,
    record_field_id, record_type_id, reference_id, scope_id, ArithmeticIntent,
    ArtifactRepresentation, BodyBlock, BodyBoundedIteration, BodyCyclePolicy, BodyOperation,
    BodyOperationKind, BodyParameter, BodyTerminator, BodyType, BodyValue,
    BoundedIterationCompletion, CompilationStatus, CompilationStudyRequest, CompilationStudyResult,
    CompilerArtifactRef, CompilerNodeProfile, CompilerPassExecutionObservation, ContractClause,
    ContractKind, Effect, FailureMode, FiniteType, FiniteVariant, Function, FunctionBody,
    IntegerType, Intent, IterationDomain, MachineIntentSpec, MachinePreference, Program,
    RecordField, RecordType, Requirement, ResolutionProvenance, SemanticBinding,
    SemanticBindingKind, SemanticBindingTable, SemanticGraph, SemanticId, SemanticIdentities,
    SemanticNamespace, SemanticReference, SemanticScope, TransformationEdge, TransformationStatus,
    ValidationReport, Value, EXECUTABLE_BODY_SCHEMA_VERSION,
    SOURCE_PROFILE_0_4_MAX_ITERATION_BOUND, SUPPORTED_SCHEMA_VERSION,
};
use mncs_syntax::{
    parse, AbstractSyntaxTree, AstBinaryOp, AstExpr, AstFunction, AstStmt, ConcreteSyntaxTree,
    DiagnosticSeverity, DiagnosticStage, LexedDocument, ParseOutput, SourceArtifactKind,
    SourceDiagnostic, SourceEnvelope, SourceSpan, SpannedText, AST_SCHEMA_VERSION,
    CST_SCHEMA_VERSION, LEXICAL_SCHEMA_VERSION, SOURCE_ENVELOPE_SCHEMA_VERSION,
    SOURCE_PROFILE_VERSION_0_4, SOURCE_PROFILE_VERSION_0_9,
};
use serde::Serialize;

fn trace_timing(stage: &str, started: Instant) {
    mncs_model::record_stage(stage, started.elapsed());
    if std::env::var_os("MNCS_TIMINGS").is_some() {
        eprintln!(
            "mncs-timing stage={} elapsed_ms={}",
            stage,
            started.elapsed().as_millis()
        );
    }
}

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
    /// Compact, versioned semantic binding substrate used by elaboration,
    /// tooling, and downstream identity/evidence consumers.
    pub binding_table: Option<SemanticBindingTable>,
    /// Authoritative name resolutions recorded during elaboration. Resolutions
    /// are recorded best-effort, so a partially valid document still exposes
    /// the occurrences that elaboration resolved successfully.
    pub name_resolutions: NameResolutionIndex,
    /// Exact source and semantic identities consumed by `use` elaboration.
    /// This is provenance, not a second resolver: the compiler's linked
    /// program remains authoritative for binding and execution.
    pub module_resolutions: Vec<ModuleResolution>,
    pub artifacts: Vec<CompilerArtifactRef>,
    pub diagnostics: Vec<SourceDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleResolution {
    pub requested_module: String,
    pub declared_module: String,
    pub module_identity: SemanticId,
    pub source_identity: String,
    pub source_logical_name: String,
    pub semantic_fingerprint: Option<String>,
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
        let started = Instant::now();
        let ParseOutput {
            lexical,
            cst,
            ast,
            diagnostics,
        } = parse(&envelope);
        mncs_model::record_counter("source_parse");
        trace_timing("parse", started);
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
        let mut binding_table = None;
        let mut module_resolutions = Vec::new();
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
                let elaboration_started = Instant::now();
                let elaboration = elaborate_program_with_resolver_and_modules(tree, resolver);
                trace_timing("elaboration", elaboration_started);
                match elaboration {
                    (Ok(elaborated), resolutions, resolved_modules) => {
                        module_resolutions = resolved_modules;
                        let report = elaborated.validate();
                        binding_table = elaborated.binding_table.clone();
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
                    (Err(mut errors), resolutions, resolved_modules) => {
                        module_resolutions = resolved_modules;
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
            binding_table,
            name_resolutions,
            module_resolutions,
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
        study = self.with_front_end_observations(study, &front_end);
        SourceStudyOutput {
            front_end,
            study: Some(study),
        }
    }

    /// Add source-front-end observations to a study built from an existing
    /// compilation result. This keeps source provenance complete without
    /// forcing callers to resolve and compile the same module graph again.
    pub fn with_front_end_observations(
        &self,
        mut study: CompilationStudyResult,
        front_end: &SourceFrontEndResult,
    ) -> CompilationStudyResult {
        let front_end_fingerprints = front_end
            .artifacts
            .iter()
            .map(|artifact| (artifact.representation, artifact.fingerprint.clone()))
            .collect::<BTreeMap<_, _>>();
        study.stage_fingerprints.extend(front_end_fingerprints);
        let mut executions = self.front_end_pass_executions(front_end);
        executions.append(&mut study.pass_executions);
        study.pass_executions = executions;
        study.compilation_status = if front_end.is_valid() {
            study.compilation_status
        } else {
            CompilationStatus::Failed
        };
        study.seal();
        study
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
    let (result, resolutions, _) = elaborate_program_with_resolver_and_modules(ast, resolver);
    (result, resolutions)
}

/// Elaborates `ast` together with its transitive imports and records the exact
/// source and semantic identity selected for every resolved module. The
/// provenance is deliberately returned separately from the program so callers
/// can audit resolution without re-implementing linker behavior.
pub fn elaborate_program_with_resolver_and_modules(
    ast: &AbstractSyntaxTree,
    resolver: &dyn ModuleResolver,
) -> (
    Result<Program, Vec<SourceDiagnostic>>,
    Vec<NameResolution>,
    Vec<ModuleResolution>,
) {
    let recording_resolver = RecordingResolver {
        inner: resolver,
        sources: RefCell::new(BTreeMap::new()),
    };
    let mut diagnostics = Vec::new();
    let mut resolutions = Vec::new();
    let mut elaborated = BTreeMap::new();
    let mut declaration_spans = BTreeMap::new();
    let mut visiting = BTreeSet::new();
    let result = if let Err(mut errors) = elaborate_import_closure(
        ast,
        &recording_resolver,
        &mut elaborated,
        &mut declaration_spans,
        &mut visiting,
    ) {
        diagnostics.append(&mut errors);
        Err(diagnostics)
    } else {
        match link_module_with_closure(
            ast,
            &elaborated,
            &mut declaration_spans,
            &mut resolutions,
            &ast.module.text,
        ) {
            Ok(program) => match mncs_model::generics::specialize_program(&program) {
                Ok(specialized) => Ok(specialized),
                Err(diags) => {
                    let mut errors: Vec<SourceDiagnostic> = diags
                        .into_iter()
                        .map(|d| elaboration_diagnostic(&d.code, d.message, ast.module.span))
                        .collect();
                    diagnostics.append(&mut errors);
                    Err(diagnostics)
                }
            },
            Err(mut errors) => {
                diagnostics.append(&mut errors);
                Err(diagnostics)
            }
        }
    };
    let sources = recording_resolver.sources.into_inner();
    let module_resolutions = elaborated
        .into_iter()
        .filter_map(|(requested_module, program)| {
            let source = sources.get(&requested_module)?;
            Some(ModuleResolution {
                requested_module,
                declared_module: program.module.clone(),
                module_identity: module_id(&program.module),
                source_identity: source.identity.clone(),
                source_logical_name: source.logical_name.clone(),
                semantic_fingerprint: program.content_fingerprint().ok(),
            })
        })
        .collect();
    (result, resolutions, module_resolutions)
}

fn module_names_compatible(requested: &str, declared: &str) -> bool {
    if requested == declared {
        return true;
    }
    strip_version_tail(requested) == Some(declared)
        || strip_version_tail(declared) == Some(requested)
}

fn strip_version_tail(name: &str) -> Option<&str> {
    let (head, tail) = name.rsplit_once('.')?;
    let version = tail.strip_prefix('v')?;
    (!version.is_empty() && version.chars().all(|character| character.is_ascii_digit()))
        .then_some(head)
}

struct RecordingResolver<'a> {
    inner: &'a dyn ModuleResolver,
    sources: RefCell<BTreeMap<String, SourceEnvelope>>,
}

impl ModuleResolver for RecordingResolver<'_> {
    fn resolve(&self, module: &str) -> Option<SourceEnvelope> {
        let source = self.inner.resolve(module)?;
        self.sources
            .borrow_mut()
            .entry(module.to_owned())
            .or_insert_with(|| source.clone());
        Some(source)
    }
}

#[derive(Debug, Clone)]
struct ImportedModule {
    requested_name: String,
    program: Program,
    declarations: DeclarationSpans,
}

#[derive(Default)]
struct MergedContext {
    finite_types_by_name: BTreeMap<String, FiniteType>,
    record_types_by_name: BTreeMap<String, RecordType>,
    signatures: BTreeMap<String, FunctionSignature>,
    /// Profile 0.9 routes retain the namespace/alias key instead of folding
    /// declarations into one global spelling table.
    qualified_finite_types: BTreeMap<String, FiniteType>,
    qualified_record_types: BTreeMap<String, RecordType>,
    qualified_signatures: BTreeMap<String, FunctionSignature>,
    namespace_aliases: BTreeMap<String, String>,
    direct_imports: BTreeSet<String>,
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
        if !module_names_compatible(&dependency_name, &dependency_ast.module.text) {
            return Err(vec![elaboration_diagnostic(
                "MNE180",
                format!(
                    "resolver returned incompatible module '{}' for requested module '{}'",
                    dependency_ast.module.text, dependency_name
                ),
                use_decl.module.span,
            )]);
        }
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
            &dependency_name,
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
    declaration_spans: &mut BTreeMap<String, DeclarationSpans>,
    resolutions: &mut Vec<NameResolution>,
    declaration_key: &str,
) -> Result<Program, Vec<SourceDiagnostic>> {
    let mut diagnostics = Vec::new();
    let mut imported = Vec::new();
    let mut seen = BTreeSet::new();
    for use_decl in &ast.uses {
        let name = use_decl.module.text.clone();
        if !seen.insert(name.clone()) {
            // Re-importing one module under a second alias is intentional and
            // preserves both qualification routes. The module is linked once;
            // `namespace_routes` registers every spelling below.
            continue;
        }
        let _ = &name;
        match elaborated.get(&name) {
            Some(program) => imported.push(ImportedModule {
                requested_name: name.clone(),
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
    let mut linked_declarations = DeclarationSpans::from_ast(ast);
    linked_declarations.merge_missing(&context.imported_declarations);
    declaration_spans.insert(declaration_key.to_owned(), linked_declarations);
    let mut program = elaborate_linked_module(ast, &context, resolutions)?;
    if mncs_syntax::profile_at_least(&ast.language_version.text, SOURCE_PROFILE_VERSION_0_9) {
        let imported_tables = imported
            .iter()
            .filter_map(|module| module.program.binding_table.as_ref())
            .collect::<Vec<_>>();
        program.binding_table = Some(build_binding_table(&program, resolutions, &imported_tables));
    }
    Ok(program)
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
    let namespaced =
        mncs_syntax::profile_at_least(&ast.language_version.text, SOURCE_PROFILE_VERSION_0_9);

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

    // Register explicit and implicit qualification routes before declarations.
    // An alias is a namespace route only; it never grants authority.
    if namespaced {
        for use_decl in &ast.uses {
            let Some(module) = imported
                .iter()
                .find(|module| module.requested_name == use_decl.module.text)
            else {
                continue;
            };
            let route = use_decl
                .alias
                .as_ref()
                .map_or_else(|| use_decl.module.text.clone(), |alias| alias.text.clone());
            if context
                .namespace_aliases
                .insert(route.clone(), module.program.module.clone())
                .is_some()
            {
                diagnostics.push(elaboration_diagnostic(
                    "MNE181",
                    format!("namespace alias '{route}' is declared more than once"),
                    use_decl.span,
                ));
            }
            context.direct_imports.insert(module.program.module.clone());
            if use_decl.alias.is_some()
                && (context.function_names.contains(&route)
                    || context.finite_type_names.contains(&route)
                    || context.record_type_names.contains(&route))
            {
                diagnostics.push(elaboration_diagnostic(
                    "MNE182",
                    format!("namespace alias '{route}' collides with a local declaration"),
                    use_decl.span,
                ));
            }
        }
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
        if namespaced {
            let routes = namespace_routes(ast, module);
            for route in routes {
                for (name, span) in &module.declarations.functions {
                    context
                        .imported_declarations
                        .qualified_functions
                        .entry(format!("{route}.{name}"))
                        .or_insert(*span);
                }
                for (name, span) in &module.declarations.finite_types {
                    context
                        .imported_declarations
                        .qualified_finite_types
                        .entry(format!("{route}.{name}"))
                        .or_insert(*span);
                }
                for ((type_name, variant), span) in &module.declarations.finite_variants {
                    context
                        .imported_declarations
                        .qualified_finite_variants
                        .entry((format!("{route}.{type_name}"), variant.clone()))
                        .or_insert(*span);
                }
                for (name, span) in &module.declarations.record_types {
                    context
                        .imported_declarations
                        .qualified_record_types
                        .entry(format!("{route}.{name}"))
                        .or_insert(*span);
                }
                for ((type_name, field_name), span) in &module.declarations.record_fields {
                    context
                        .imported_declarations
                        .qualified_record_fields
                        .entry((format!("{route}.{type_name}"), field_name.clone()))
                        .or_insert(*span);
                }
            }
        }
        for finite_type in &module.program.finite_types {
            if namespaced {
                for route in namespace_routes(ast, module) {
                    context
                        .qualified_finite_types
                        .insert(format!("{route}.{}", finite_type.name), finite_type.clone());
                }
            }
            // A local declaration sharing a name with an import is always a
            // collision; the module declared both under one namespace.
            if context.finite_type_names.contains(&finite_type.name) && !namespaced {
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
                    if !namespaced {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE174",
                            format!(
                                "imported finite type '{}' collides with an existing binding",
                                finite_type.name
                            ),
                            ast.module.span,
                        ));
                    }
                } else {
                    continue;
                }
            }
            context
                .finite_types_by_name
                .insert(finite_type.name.clone(), finite_type.clone());
            if !context
                .imported_finite_types
                .iter()
                .any(|existing| existing.identity == finite_type.identity)
            {
                context.imported_finite_types.push(finite_type.clone());
            }
        }
        for record_type in &module.program.record_types {
            if namespaced {
                for route in namespace_routes(ast, module) {
                    context
                        .qualified_record_types
                        .insert(format!("{route}.{}", record_type.name), record_type.clone());
                }
            }
            if context.record_type_names.contains(&record_type.name) && !namespaced {
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
                    if !namespaced {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE175",
                            format!(
                                "imported record type '{}' collides with an existing binding",
                                record_type.name
                            ),
                            ast.module.span,
                        ));
                    }
                } else {
                    continue;
                }
            }
            context
                .record_types_by_name
                .insert(record_type.name.clone(), record_type.clone());
            if !context
                .imported_record_types
                .iter()
                .any(|existing| existing.identity == record_type.identity)
            {
                context.imported_record_types.push(record_type.clone());
            }
        }
        for function in &module.program.functions {
            let mut imported_function = function.clone();
            if namespaced {
                canonicalize_function_types(&mut imported_function, module);
            }
            let candidate_signature = signature_from_function(module, &imported_function);
            if namespaced {
                for route in namespace_routes(ast, module) {
                    context.qualified_signatures.insert(
                        format!("{route}.{}", function.name),
                        candidate_signature.clone(),
                    );
                }
            }
            if context.function_names.contains(&function.name) && !namespaced {
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
                    if !namespaced {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE176",
                            format!(
                                "imported function '{}' collides with an existing binding",
                                function.name
                            ),
                            ast.module.span,
                        ));
                    }
                } else {
                    continue;
                }
            }
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
            if !context.imported_functions.iter().any(|existing| {
                existing.name == imported_function.name
                    && existing.identity_namespace(&module.program.module)
                        == imported_function.identity_namespace(&module.program.module)
            }) {
                context.imported_functions.push(imported_function);
            }
        }
    }
    if namespaced {
        // Only a unique imported spelling is eligible for unqualified lookup.
        // A local declaration takes precedence over imported candidates; two
        // imported identities remain qualification-only.
        context.signatures.clear();
        context.finite_types_by_name.clear();
        context.record_types_by_name.clear();
        for imported in &context.imported_finite_types {
            if context.finite_type_names.contains(&imported.name) {
                continue;
            }
            let same_name = context
                .imported_finite_types
                .iter()
                .filter(|candidate| candidate.name == imported.name)
                .collect::<Vec<_>>();
            if same_name.len() == 1 {
                context
                    .finite_types_by_name
                    .insert(imported.name.clone(), imported.clone());
            }
        }
        for imported in &context.imported_record_types {
            if context.record_type_names.contains(&imported.name) {
                continue;
            }
            let same_name = context
                .imported_record_types
                .iter()
                .filter(|candidate| candidate.name == imported.name)
                .collect::<Vec<_>>();
            if same_name.len() == 1 {
                context
                    .record_types_by_name
                    .insert(imported.name.clone(), imported.clone());
            }
        }
        let mut grouped = BTreeMap::<String, Vec<FunctionSignature>>::new();
        for signature in context.qualified_signatures.values() {
            let Some(function_name) = signature.identity.0.rsplit_once("::").map(|(_, name)| name)
            else {
                continue;
            };
            grouped
                .entry(function_name.to_owned())
                .or_default()
                .push(signature.clone());
        }
        for (name, candidates) in grouped {
            let identities = candidates
                .iter()
                .map(|candidate| candidate.identity.clone())
                .collect::<BTreeSet<_>>();
            if identities.len() == 1 && !context.function_names.contains(&name) {
                context.signatures.insert(name, candidates[0].clone());
            }
        }
        context
            .signatures
            .extend(context.qualified_signatures.clone());
    }
    context
}

fn namespace_routes(ast: &AbstractSyntaxTree, module: &ImportedModule) -> Vec<String> {
    let mut routes = vec![module.program.module.clone()];
    for use_decl in ast
        .uses
        .iter()
        .filter(|use_decl| use_decl.module.text == module.requested_name)
    {
        routes.push(use_decl.module.text.clone());
        if let Some(alias) = &use_decl.alias {
            routes.push(alias.text.clone());
        }
    }
    routes.sort();
    routes.dedup();
    routes
}

/// Existing semantic IDs intentionally keep their canonical components
/// inspectable. Recovering the declaring module here lets the binding witness
/// point at the same owner without adding a second ownership field to every
/// nominal model object.
fn semantic_namespace_from_identity(identity: &SemanticId) -> String {
    let Some((_, components)) = identity.0.split_once("mncs:0.2:") else {
        return String::new();
    };
    let Some((kind, encoded)) = components.split_once(':') else {
        return String::new();
    };
    if !matches!(
        kind,
        "module" | "finite-type" | "finite-variant" | "record-type" | "record-field" | "function"
    ) {
        return String::new();
    }
    let first = encoded.split("::").next().unwrap_or_default();
    decode_identity_component(first)
}

fn decode_identity_component(encoded: &str) -> String {
    let mut decoded = String::with_capacity(encoded.len());
    let bytes = encoded.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = &encoded[index + 1..index + 3];
            if let Ok(byte) = u8::from_str_radix(hex, 16) {
                decoded.push(char::from(byte));
                index += 3;
                continue;
            }
        }
        decoded.push(char::from(bytes[index]));
        index += 1;
    }
    decoded
}

/// Builds the call-checking signature for a function declared in another
/// module. Identities stay anchored to the declaring module: a function that
/// itself arrived through linking keeps its original home namespace.
fn signature_from_function(module: &ImportedModule, function: &Function) -> FunctionSignature {
    let body_type = |name: &str| body_type_from_name(module, name, &function.generic_params);
    let mut inputs: Vec<BodyType> = function
        .inputs
        .iter()
        .map(|value| body_type(&value.value_type))
        .collect();
    let mut output = function.outputs.first().map_or_else(
        || BodyType::Named("invalid".to_owned()),
        |value| body_type(&value.value_type),
    );
    // Rehydrate generic parameter occurrences in the signature types.
    // Imported bodies already store GenericParam for direct `T` occurrences via
    // canonical_value_type, but sequences like `[T; N]` parse the element as
    // Named("T"); we transform those plus value-param bounds consistently.
    let generic_type_names: std::collections::BTreeSet<String> = function
        .generic_params
        .iter()
        .filter(|p| p.kind == mncs_model::GenericParamKind::Type)
        .map(|p| p.name.clone())
        .collect();
    let remap = |ty: BodyType| -> BodyType {
        match ty {
            BodyType::Named(n) if generic_type_names.contains(&n) => {
                BodyType::GenericParam { name: n }
            }
            BodyType::Sequence { element, bound } => {
                let new_elem = match *element {
                    BodyType::Named(n) if generic_type_names.contains(&n) => {
                        Box::new(BodyType::GenericParam { name: n })
                    }
                    other => Box::new(other),
                };
                BodyType::Sequence {
                    element: new_elem,
                    bound,
                }
            }
            other => other,
        }
    };
    inputs = inputs.into_iter().map(&remap).collect::<Vec<_>>();
    output = remap(output);
    let mut capabilities = function.capabilities.clone();
    capabilities.sort();
    capabilities.dedup();
    let home = function
        .home_module
        .as_deref()
        .unwrap_or(&module.program.module);
    FunctionSignature {
        identity: function_id(home, &function.name),
        namespace: home.to_owned(),
        generic_params: function.generic_params.clone(),
        inputs,
        output,
        capabilities,
        effects: function.effects.clone(),
    }
}

/// Resolves a type name against the merged namespace without diagnosing:
/// dependency programs are valid by the time merging happens, so unknown
/// names cannot occur here.
fn body_type_from_name(
    module: &ImportedModule,
    name: &str,
    generic_params: &[mncs_model::GenericParam],
) -> BodyType {
    if let Some(finite_type) = module
        .program
        .finite_types
        .iter()
        .find(|decl| decl.identity.0 == name)
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
        .find(|decl| decl.identity.0 == name)
    {
        return BodyType::Record {
            identity: record_type.identity.clone(),
            name: record_type.name.clone(),
        };
    }
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
    // Imported function signatures retain source-level bounded-sequence
    // spellings such as `[Status; 8]`. Resolve the element against the
    // declaring module's nominal namespace so a linked sequence has the same
    // identity-bearing type as the corresponding local sequence. Without
    // this, an imported `[Status; N]` is rehydrated as `Named("Status")` and
    // valid calls fail closed as a type mismatch.
    let finite_types = module
        .program
        .finite_types
        .iter()
        .map(|finite| (finite.name.clone(), finite.clone()))
        .collect::<BTreeMap<_, _>>();
    let record_types = module
        .program
        .record_types
        .iter()
        .map(|record| (record.name.clone(), record.clone()))
        .collect::<BTreeMap<_, _>>();
    let generic_map = generic_params
        .iter()
        .map(|parameter| (parameter.name.clone(), parameter.kind))
        .collect::<BTreeMap<_, _>>();
    let mut diagnostics = Vec::new();
    if let Some(sequence) = profile_sequence_type_with_generics(
        name,
        &finite_types,
        &record_types,
        &generic_map,
        &mut diagnostics,
        SourceSpan {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        },
    ) {
        return sequence;
    }
    BodyType::from_semantic_name(name)
}

fn canonicalize_function_types(function: &mut Function, module: &ImportedModule) {
    for value in function
        .inputs
        .iter_mut()
        .chain(function.outputs.iter_mut())
    {
        let ty = body_type_from_name(module, &value.value_type, &function.generic_params);
        value.value_type = match ty {
            BodyType::Sequence { .. } => canonical_imported_sequence_type(&value.value_type, &ty),
            BodyType::Finite { identity, .. } | BodyType::Record { identity, .. } => identity.0,
            _ => value.value_type.clone(),
        };
    }
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
    record_annotation_type_resolutions(ast, &declarations, context, resolutions);
    // Type names visible to field/payload positions include imported
    // bindings: cross-module composition requires a record field or variant
    // payload to be able to name an imported type (collisions were already
    // rejected during merging, so the union is unambiguous).
    let mut declared_finite_names: BTreeSet<String> = ast
        .finite_types
        .iter()
        .map(|finite| finite.name.text.clone())
        .collect();
    declared_finite_names.extend(context.finite_types_by_name.keys().cloned());
    declared_finite_names.extend(context.qualified_finite_types.keys().cloned());
    let mut declared_record_names: BTreeSet<String> = ast
        .record_types
        .iter()
        .map(|record| record.name.text.clone())
        .collect();
    declared_record_names.extend(context.record_types_by_name.keys().cloned());
    declared_record_names.extend(context.qualified_record_types.keys().cloned());
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
    let mut finite_types_by_name = ast
        .finite_types
        .iter()
        .filter_map(|declaration| {
            finite_types
                .iter()
                .find(|candidate| {
                    candidate.identity == finite_type_id(&ast.module.text, &declaration.name.text)
                })
                .map(|candidate| (candidate.name.clone(), candidate.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    for imported in &context.imported_finite_types {
        if !finite_types_by_name.contains_key(&imported.name)
            && context
                .imported_finite_types
                .iter()
                .filter(|candidate| candidate.name == imported.name)
                .count()
                == 1
        {
            finite_types_by_name.insert(imported.name.clone(), imported.clone());
        }
    }
    if mncs_syntax::profile_at_least(&ast.language_version.text, SOURCE_PROFILE_VERSION_0_9) {
        finite_types_by_name.extend(context.qualified_finite_types.clone());
    }
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
    // resolution runs after every valid record name is registered. Imported
    // record names participate too: a local record field may reference an
    // imported type.
    let mut provisional_names = record_types
        .iter()
        .map(|(declaration, _)| declaration.name.text.clone())
        .collect::<BTreeSet<_>>();
    provisional_names.extend(context.record_types_by_name.keys().cloned());
    provisional_names.extend(context.qualified_record_types.keys().cloned());
    let mut provisional_record_types = context.record_types_by_name.clone();
    provisional_record_types.extend(context.qualified_record_types.clone());
    // Record fields may contain bounded sequences of local records.  The
    // field-type pass below runs before `resolved_records` is finalized, so
    // seed the provisional map with the local declarations as well.  Without
    // this, `[LocalRecord; N]` was accepted in function signatures but
    // rejected when it appeared inside another record.
    for (declaration, duplicate_fields) in &record_types {
        if *duplicate_fields == 0 {
            provisional_record_types.insert(
                declaration.name.text.clone(),
                RecordType {
                    identity: record_type_id(
                        &ast.module.text,
                        &declaration.name.text,
                        &declaration
                            .fields
                            .iter()
                            .map(|field| (field.name.text.as_str(), field.value_type.text.as_str()))
                            .collect::<Vec<_>>(),
                    ),
                    name: declaration.name.text.clone(),
                    fields: declaration
                        .fields
                        .iter()
                        .map(|field| RecordField {
                            name: field.name.text.clone(),
                            field_type: field.value_type.text.clone(),
                        })
                        .collect(),
                },
            );
        }
    }
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
                // Bounded-sequence spellings (Profile 0.7) resolve against
                // the same provisional namespaces as their element types.
                && profile_sequence_type(
                    &field.value_type.text,
                    &finite_types_by_name,
                    &provisional_record_types,
                )
                .is_none()
            {
                diagnostics.push(elaboration_diagnostic(
                    "MNE153",
                    "record field type does not name a supported scalar, finite, or declared record type",
                    field.value_type.span,
                ));
                continue;
            }
            let field_type = profile_type(
                &field.value_type.text,
                field.value_type.span,
                &finite_types_by_name,
                &provisional_record_types,
                &mut diagnostics,
            );
            fields.push(RecordField {
                name: field.name.text.clone(),
                field_type: canonical_value_type(&field.value_type.text, &field_type),
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
    let mut all_records = resolved_records.clone();
    for imported in &context.imported_record_types {
        all_records.push(imported.clone());
    }
    let mut record_types_by_name = ast
        .record_types
        .iter()
        .filter_map(|declaration| {
            all_records
                .iter()
                .find(|candidate| {
                    candidate.identity
                        == record_type_id(&ast.module.text, &declaration.name.text, &[])
                })
                .map(|candidate| (candidate.name.clone(), candidate.clone()))
        })
        .collect::<BTreeMap<_, _>>();
    for local in &resolved_records {
        record_types_by_name.insert(local.name.clone(), local.clone());
    }
    for imported in &context.imported_record_types {
        if !record_types_by_name.contains_key(&imported.name)
            && context
                .imported_record_types
                .iter()
                .filter(|candidate| candidate.name == imported.name)
                .count()
                == 1
        {
            record_types_by_name.insert(imported.name.clone(), imported.clone());
        }
    }
    if mncs_syntax::profile_at_least(&ast.language_version.text, SOURCE_PROFILE_VERSION_0_9) {
        record_types_by_name.extend(context.qualified_record_types.clone());
    }
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
            &context.namespace_aliases,
            &context.direct_imports,
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
            binding_table: None,
            functions: all_functions,
            generic_specializations: Vec::new(),
        })
    } else {
        Err(diagnostics)
    }
}

fn build_binding_table(
    program: &Program,
    resolutions: &mut [NameResolution],
    imported_tables: &[&SemanticBindingTable],
) -> SemanticBindingTable {
    let root_namespace = module_id(&program.module);
    let mut namespace_names = BTreeMap::<SemanticId, String>::new();
    namespace_names.insert(root_namespace.clone(), program.module.clone());
    for dependency in &program.dependencies {
        let name = semantic_namespace_from_identity(dependency);
        if !name.is_empty() {
            namespace_names.insert(dependency.clone(), name);
        }
    }
    for identity in program.finite_types.iter().map(|item| &item.identity) {
        let name = semantic_namespace_from_identity(identity);
        if !name.is_empty() {
            namespace_names.insert(module_id(&name), name);
        }
    }
    for identity in program.record_types.iter().map(|item| &item.identity) {
        let name = semantic_namespace_from_identity(identity);
        if !name.is_empty() {
            namespace_names.insert(module_id(&name), name);
        }
    }
    for function in &program.functions {
        let identity = function_id(function.identity_namespace(&program.module), &function.name);
        let name = semantic_namespace_from_identity(&identity);
        if !name.is_empty() {
            namespace_names.insert(module_id(&name), name);
        }
    }

    let mut namespaces = namespace_names
        .iter()
        .map(|(identity, name)| SemanticNamespace {
            identity: identity.clone(),
            name: name.clone(),
        })
        .collect::<Vec<_>>();
    for table in imported_tables {
        namespaces.extend(table.namespaces.clone());
    }
    let mut scopes = Vec::new();
    for namespace in namespace_names.keys() {
        scopes.push(SemanticScope {
            identity: scope_id(namespace, namespace, "module"),
            namespace: namespace.clone(),
            parent: None,
            owner: None,
            path: "module".to_owned(),
        });
    }
    for function in &program.functions {
        let name = function.identity_namespace(&program.module);
        let namespace = module_id(name);
        let owner = function_id(name, &function.name);
        scopes.push(SemanticScope {
            identity: scope_id(&namespace, &owner, "body"),
            namespace,
            parent: None,
            owner: Some(owner),
            path: "body".to_owned(),
        });
    }
    for resolution in resolutions.iter() {
        if let (Some(scope), Some(namespace)) = (&resolution.scope, &resolution.namespace) {
            scopes.push(SemanticScope {
                identity: scope.clone(),
                namespace: namespace.clone(),
                parent: None,
                owner: None,
                path: "lexical".to_owned(),
            });
        }
    }
    for table in imported_tables {
        scopes.extend(table.scopes.clone());
    }

    let mut bindings = Vec::new();
    for function in &program.functions {
        let namespace_name = function.identity_namespace(&program.module);
        let namespace = module_id(namespace_name);
        let module_scope = scope_id(&namespace, &namespace, "module");
        let identity = function_id(namespace_name, &function.name);
        bindings.push(SemanticBinding {
            identity: binding_id_for(&module_scope, "function", &identity),
            spelling: function.name.clone(),
            namespace: namespace.clone(),
            scope: module_scope,
            declaration: identity,
            kind: SemanticBindingKind::Function,
        });
    }
    for finite_type in &program.finite_types {
        let namespace_name = semantic_namespace_from_identity(&finite_type.identity);
        let namespace = module_id(&namespace_name);
        let module_scope = scope_id(&namespace, &namespace, "module");
        bindings.push(SemanticBinding {
            identity: binding_id_for(&module_scope, "finite-type", &finite_type.identity),
            spelling: finite_type.name.clone(),
            namespace: namespace.clone(),
            scope: module_scope.clone(),
            declaration: finite_type.identity.clone(),
            kind: SemanticBindingKind::FiniteType,
        });
        for variant in &finite_type.variants {
            bindings.push(SemanticBinding {
                identity: binding_id_for(&module_scope, "finite-variant", &variant.identity),
                spelling: variant.name.clone(),
                namespace: namespace.clone(),
                scope: module_scope.clone(),
                declaration: variant.identity.clone(),
                kind: SemanticBindingKind::FiniteVariant,
            });
        }
    }
    for record_type in &program.record_types {
        let namespace_name = semantic_namespace_from_identity(&record_type.identity);
        let namespace = module_id(&namespace_name);
        let module_scope = scope_id(&namespace, &namespace, "module");
        bindings.push(SemanticBinding {
            identity: binding_id_for(&module_scope, "record-type", &record_type.identity),
            spelling: record_type.name.clone(),
            namespace: namespace.clone(),
            scope: module_scope.clone(),
            declaration: record_type.identity.clone(),
            kind: SemanticBindingKind::RecordType,
        });
        for field in &record_type.fields {
            let identity = record_field_id(&namespace_name, &record_type.name, &field.name);
            bindings.push(SemanticBinding {
                identity: binding_id_for(&module_scope, "record-field", &identity),
                spelling: field.name.clone(),
                namespace: namespace.clone(),
                scope: module_scope.clone(),
                declaration: identity,
                kind: SemanticBindingKind::RecordField,
            });
        }
    }
    for resolution in resolutions.iter() {
        let (Some(binding), Some(namespace), Some(scope), Some(declaration)) = (
            &resolution.binding,
            &resolution.namespace,
            &resolution.scope,
            &resolution.declaration_identity,
        ) else {
            continue;
        };
        let kind = match resolution.kind {
            ResolvedNameKind::Function => SemanticBindingKind::Function,
            ResolvedNameKind::Parameter => SemanticBindingKind::Parameter,
            ResolvedNameKind::Binding => SemanticBindingKind::Local,
            ResolvedNameKind::IterationState => SemanticBindingKind::IterationState,
            ResolvedNameKind::FiniteType => SemanticBindingKind::FiniteType,
            ResolvedNameKind::FiniteVariant => SemanticBindingKind::FiniteVariant,
            ResolvedNameKind::RecordType => SemanticBindingKind::RecordType,
            ResolvedNameKind::RecordField => SemanticBindingKind::RecordField,
        };
        bindings.push(SemanticBinding {
            identity: binding.clone(),
            spelling: resolution.path.first().cloned().unwrap_or_default(),
            namespace: namespace.clone(),
            scope: scope.clone(),
            declaration: declaration.clone(),
            kind,
        });
    }
    for table in imported_tables {
        bindings.extend(table.bindings.clone());
    }

    let mut reference_slots = BTreeMap::<(SemanticId, SemanticId), usize>::new();
    let mut references = Vec::new();
    for resolution in resolutions.iter_mut() {
        let (Some(binding), Some(namespace), Some(scope), Some(provenance)) = (
            &resolution.binding,
            &resolution.namespace,
            &resolution.scope,
            &resolution.provenance,
        ) else {
            continue;
        };
        let key = (namespace.clone(), scope.clone());
        let slot = reference_slots.entry(key).or_default();
        let identity = reference_id(namespace, scope, *slot);
        *slot += 1;
        resolution.reference = Some(identity.clone());
        references.push(SemanticReference {
            identity,
            spelling: resolution.path.first().cloned().unwrap_or_default(),
            occurrence_start: resolution.occurrence.start,
            occurrence_end: resolution.occurrence.end,
            namespace: namespace.clone(),
            scope: scope.clone(),
            binding: binding.clone(),
            path: resolution.path.clone(),
            provenance: provenance.clone(),
        });
    }
    for table in imported_tables {
        references.extend(table.references.clone());
    }
    SemanticBindingTable::new(namespaces, scopes, bindings, references)
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
    qualified_functions: BTreeMap<String, SourceSpan>,
    qualified_finite_types: BTreeMap<String, SourceSpan>,
    qualified_finite_variants: BTreeMap<(String, String), SourceSpan>,
    qualified_record_types: BTreeMap<String, SourceSpan>,
    qualified_record_fields: BTreeMap<(String, String), SourceSpan>,
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
        for (name, span) in &imported.qualified_functions {
            self.qualified_functions
                .entry(name.clone())
                .or_insert(*span);
        }
        for (name, span) in &imported.qualified_finite_types {
            self.qualified_finite_types
                .entry(name.clone())
                .or_insert(*span);
        }
        for (name, span) in &imported.qualified_finite_variants {
            self.qualified_finite_variants
                .entry(name.clone())
                .or_insert(*span);
        }
        for (name, span) in &imported.qualified_record_types {
            self.qualified_record_types
                .entry(name.clone())
                .or_insert(*span);
        }
        for (name, span) in &imported.qualified_record_fields {
            self.qualified_record_fields
                .entry(name.clone())
                .or_insert(*span);
        }
    }
}

/// Record type-annotation occurrences (parameter, output, binding, and carried
/// state types). Annotation positions are syntactically unambiguous; the only
/// decision is whether the annotated name denotes a declared nominal type.
fn record_annotation_type_resolutions(
    ast: &AbstractSyntaxTree,
    declarations: &DeclarationSpans,
    context: &MergedContext,
    resolutions: &mut Vec<NameResolution>,
) {
    for function in &ast.functions {
        for parameter in function.inputs.iter().chain(&function.outputs) {
            record_annotation(
                &parameter.value_type,
                &function.name.text,
                ast,
                declarations,
                context,
                resolutions,
            );
        }
        record_annotation_block(
            &function.body.statements,
            &function.name.text,
            ast,
            declarations,
            context,
            resolutions,
        );
    }
}

fn record_annotation_block(
    statements: &[AstStmt],
    function: &str,
    ast: &AbstractSyntaxTree,
    declarations: &DeclarationSpans,
    context: &MergedContext,
    resolutions: &mut Vec<NameResolution>,
) {
    for statement in statements {
        match statement {
            AstStmt::Let { value_type, .. } => {
                record_annotation(
                    value_type,
                    function,
                    ast,
                    declarations,
                    context,
                    resolutions,
                );
            }
            AstStmt::If {
                then_body,
                else_body,
                ..
            } => {
                record_annotation_block(
                    then_body,
                    function,
                    ast,
                    declarations,
                    context,
                    resolutions,
                );
                record_annotation_block(
                    else_body,
                    function,
                    ast,
                    declarations,
                    context,
                    resolutions,
                );
            }
            AstStmt::BoundedIteration {
                state_type, body, ..
            } => {
                record_annotation(
                    state_type,
                    function,
                    ast,
                    declarations,
                    context,
                    resolutions,
                );
                record_annotation_block(body, function, ast, declarations, context, resolutions);
            }
            AstStmt::Fail { .. } | AstStmt::Return { .. } => {}
        }
    }
}

fn annotation_kind(name: &str, declarations: &DeclarationSpans) -> Option<ResolvedNameKind> {
    if declarations.finite_types.contains_key(name)
        || declarations.qualified_finite_types.contains_key(name)
    {
        Some(ResolvedNameKind::FiniteType)
    } else if declarations.record_types.contains_key(name)
        || declarations.qualified_record_types.contains_key(name)
    {
        Some(ResolvedNameKind::RecordType)
    } else {
        None
    }
}

fn record_annotation(
    name: &SpannedText,
    function: &str,
    ast: &AbstractSyntaxTree,
    declarations: &DeclarationSpans,
    context: &MergedContext,
    resolutions: &mut Vec<NameResolution>,
) {
    let Some(kind) = annotation_kind(&name.text, declarations) else {
        return;
    };
    let declaration = match kind {
        ResolvedNameKind::FiniteType => declarations
            .qualified_finite_types
            .get(&name.text)
            .or_else(|| declarations.finite_types.get(&name.text)),
        _ => declarations
            .qualified_record_types
            .get(&name.text)
            .or_else(|| declarations.record_types.get(&name.text)),
    }
    .copied();
    let Some(declaration) = declaration else {
        return;
    };
    let Some((declaration_identity, declaration_namespace)) =
        annotation_identity(name, kind, ast, context)
    else {
        resolutions.push(NameResolution::new(name.span, declaration, kind));
        return;
    };
    let source_namespace = module_id(&ast.module.text);
    let owner = function_id(&ast.module.text, function);
    let scope = scope_id(&source_namespace, &owner, "body");
    let declaration_namespace_id = module_id(&declaration_namespace);
    let module_scope = scope_id(
        &declaration_namespace_id,
        &declaration_namespace_id,
        "module",
    );
    let binding_kind = match kind {
        ResolvedNameKind::FiniteType => "finite-type",
        ResolvedNameKind::RecordType => "record-type",
        _ => "nominal",
    };
    let mut path = name.text.split('.').map(str::to_owned).collect::<Vec<_>>();
    if path.is_empty() {
        path.push(name.text.clone());
    }
    if declaration_namespace != ast.module.text {
        path.push(declaration_namespace.clone());
    }
    path.push(declaration_identity.0.clone());
    let provenance = if declaration_namespace == ast.module.text {
        ResolutionProvenance::Local
    } else if path
        .first()
        .is_some_and(|route| context.namespace_aliases.contains_key(route))
    {
        ResolutionProvenance::Aliased
    } else if name.text.contains('.') {
        ResolutionProvenance::Qualified
    } else if context.direct_imports.contains(&declaration_namespace) {
        ResolutionProvenance::DirectImport
    } else {
        ResolutionProvenance::TransitiveImport
    };
    resolutions.push(
        NameResolution::new(name.span, declaration, kind).with_binding_metadata(
            binding_id_for(&module_scope, binding_kind, &declaration_identity),
            source_namespace,
            scope,
            path,
            provenance,
            declaration_identity,
        ),
    );
}

fn annotation_identity(
    name: &SpannedText,
    kind: ResolvedNameKind,
    ast: &AbstractSyntaxTree,
    context: &MergedContext,
) -> Option<(SemanticId, String)> {
    match kind {
        ResolvedNameKind::FiniteType => {
            if let Some(finite_type) = context
                .qualified_finite_types
                .get(&name.text)
                .or_else(|| context.finite_types_by_name.get(&name.text))
            {
                return Some((
                    finite_type.identity.clone(),
                    semantic_namespace_from_identity(&finite_type.identity),
                ));
            }
            ast.finite_types
                .iter()
                .find(|declaration| declaration.name.text == name.text)
                .map(|declaration| {
                    (
                        finite_type_id(&ast.module.text, &declaration.name.text),
                        ast.module.text.clone(),
                    )
                })
        }
        ResolvedNameKind::RecordType => {
            if let Some(record_type) = context
                .qualified_record_types
                .get(&name.text)
                .or_else(|| context.record_types_by_name.get(&name.text))
            {
                return Some((
                    record_type.identity.clone(),
                    semantic_namespace_from_identity(&record_type.identity),
                ));
            }
            ast.record_types
                .iter()
                .find(|declaration| declaration.name.text == name.text)
                .map(|declaration| {
                    let fields = declaration
                        .fields
                        .iter()
                        .map(|field| (field.name.text.as_str(), field.value_type.text.as_str()))
                        .collect::<Vec<_>>();
                    (
                        record_type_id(&ast.module.text, &declaration.name.text, &fields),
                        ast.module.text.clone(),
                    )
                })
        }
        _ => None,
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
    namespace_aliases: &BTreeMap<String, String>,
    direct_imports: &BTreeSet<String>,
    resolutions: &mut Vec<NameResolution>,
) -> Result<Function, Vec<SourceDiagnostic>> {
    let mut diagnostics = Vec::new();
    // ---- Generic parameter elaboration (Profile 0.10) ----
    let generic_params = if function.generic_params.is_empty() {
        Vec::new()
    } else {
        if !mncs_syntax::profile_at_least(
            &ast.language_version.text,
            mncs_syntax::SOURCE_PROFILE_VERSION_0_10,
        ) {
            diagnostics.push(elaboration_diagnostic(
                "MNP184",
                "generic parameters require source profile 0.10 or later",
                function.generic_params[0].span,
            ));
        }
        let mut parsed: Vec<mncs_model::GenericParam> = Vec::new();
        let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for gp in &function.generic_params {
            let name = gp.name.text.clone();
            if name.trim().is_empty() {
                diagnostics.push(elaboration_diagnostic(
                    "MNE228",
                    "generic parameter name must not be empty",
                    gp.name.span,
                ));
                continue;
            }
            if !seen.insert(name.clone()) {
                diagnostics.push(elaboration_diagnostic(
                    "MNE228",
                    format!("duplicate generic parameter '{name}'"),
                    gp.name.span,
                ));
                continue;
            }
            let constraint = gp
                .constraint
                .as_ref()
                .map(|constraint| constraint.text.as_str());
            if gp
                .constraint
                .as_ref()
                .is_some_and(|c| c.text.contains("->"))
            {
                diagnostics.push(elaboration_diagnostic(
                    "MNE229",
                    "higher-kinded generic parameters are not supported in this tranche",
                    gp.span,
                ));
                continue;
            }
            let kind = match constraint {
                Some("Nat") => mncs_model::GenericParamKind::Nat,
                Some("Type") | None => mncs_model::GenericParamKind::Type,
                Some(other) => {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE229",
                        format!(
                            "unsupported generic constraint '{other}'; expected 'Type' or 'Nat'"
                        ),
                        gp.constraint.as_ref().unwrap().span,
                    ));
                    continue;
                }
            };
            parsed.push(mncs_model::GenericParam { name, kind });
        }
        // Validate generic param names don't shadow function parameter names (value namespace)
        for gp in &parsed {
            if function.inputs.iter().any(|p| p.name.text == gp.name)
                || function.outputs.iter().any(|p| p.name.text == gp.name)
            {
                diagnostics.push(elaboration_diagnostic(
                    "MNE228",
                    format!(
                        "generic parameter '{}' conflicts with a function parameter name",
                        gp.name
                    ),
                    function.name.span,
                ));
            }
        }
        parsed
    };
    let generic_map: std::collections::BTreeMap<String, mncs_model::GenericParamKind> =
        generic_params
            .iter()
            .map(|p| (p.name.clone(), p.kind))
            .collect();
    if function.outputs.len() != 1 {
        diagnostics.push(elaboration_diagnostic(
            "MNE101",
            "the current source profiles require exactly one named output",
            function.span,
        ));
        return Err(diagnostics);
    }
    let mut inputs = function
        .inputs
        .iter()
        .map(|input| Value {
            name: input.name.text.clone(),
            value_type: input.value_type.text.clone(),
        })
        .collect::<Vec<_>>();
    let mut outputs = function
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
            ty: profile_type_with_generics(
                &input.value_type.text,
                input.value_type.span,
                finite_types,
                record_types,
                &generic_map,
                &mut diagnostics,
            ),
        })
        .collect::<Vec<_>>();
    let output_type = profile_type_with_generics(
        &function.outputs[0].value_type.text,
        function.outputs[0].value_type.span,
        finite_types,
        record_types,
        &generic_map,
        &mut diagnostics,
    );
    // Keep pre-0.9 source spellings stable. Qualified nominal types need an
    // identity-bearing spelling in the semantic Program so body/IR/SSA
    // validation cannot accidentally collapse two imported `Thing` types.
    for (value, parameter) in inputs.iter_mut().zip(&parameters) {
        value.value_type = canonical_value_type(&value.value_type, &parameter.ty);
    }
    if let Some(output) = outputs.first_mut() {
        output.value_type = canonical_value_type(&output.value_type, &output_type);
    }
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
    let mut env = BindingEnv::new(&ast.module.text, &function.name.text);
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
            ast.module.text.clone(),
            finite_types,
            record_types,
            signatures,
            declarations,
            namespace_aliases,
            direct_imports,
            generic_map.clone(),
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
        generic_params: generic_params.clone(),
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
            generic_params: generic_params.clone(),
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
    namespace: String,
    generic_params: Vec<mncs_model::GenericParam>,
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
        // Generic params for this signature (used to resolve T, N in inputs/outputs)
        let generic_params = if function.generic_params.is_empty() {
            Vec::new()
        } else {
            let mut pmap = Vec::new();
            for gp in &function.generic_params {
                let kind = match gp.constraint.as_ref().map(|c| c.text.as_str()) {
                    Some("Nat") => mncs_model::GenericParamKind::Nat,
                    _ => mncs_model::GenericParamKind::Type,
                };
                pmap.push(mncs_model::GenericParam {
                    name: gp.name.text.clone(),
                    kind,
                });
            }
            pmap
        };
        let generic_map: std::collections::BTreeMap<String, mncs_model::GenericParamKind> =
            generic_params
                .iter()
                .map(|p| (p.name.clone(), p.kind))
                .collect();
        let inputs = function
            .inputs
            .iter()
            .map(|input| {
                profile_type_with_generics(
                    &input.value_type.text,
                    input.value_type.span,
                    finite_types,
                    record_types,
                    &generic_map,
                    diagnostics,
                )
            })
            .collect();
        let output = function.outputs.first().map_or_else(
            || BodyType::Named("invalid".to_owned()),
            |output| {
                profile_type_with_generics(
                    &output.value_type.text,
                    output.value_type.span,
                    finite_types,
                    record_types,
                    &generic_map,
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
            namespace: ast.module.text.clone(),
            generic_params,
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
        AstExpr::Index { base, index, .. } => {
            calls_in_expr(base, calls);
            calls_in_expr(index, calls);
        }
        AstExpr::Slice {
            base, start, end, ..
        } => {
            calls_in_expr(base, calls);
            calls_in_expr(start, calls);
            calls_in_expr(end, calls);
        }
        AstExpr::Cast { value, .. } => calls_in_expr(value, calls),
        AstExpr::Select {
            condition,
            when_true,
            when_false,
            ..
        } => {
            calls_in_expr(condition, calls);
            calls_in_expr(when_true, calls);
            calls_in_expr(when_false, calls);
        }
        AstExpr::SequenceReplace {
            sequence,
            index,
            element,
            ..
        } => {
            calls_in_expr(sequence, calls);
            calls_in_expr(index, calls);
            calls_in_expr(element, calls);
        }
        AstExpr::VectorIntrinsic { arguments, .. } => {
            for argument in arguments {
                calls_in_expr(argument, calls);
            }
        }
        AstExpr::SequenceLiteral { elements, .. } => {
            for element in elements {
                calls_in_expr(element, calls);
            }
        }
        AstExpr::Name(_)
        | AstExpr::QualifiedPath { .. }
        | AstExpr::Integer { .. }
        | AstExpr::Boolean { .. }
        | AstExpr::FiniteVariant { .. } => {}
    }
}

struct BindingEnv {
    scopes: Vec<std::collections::BTreeMap<String, (ResolvedBinding, SourceSpan, BoundNameKind)>>,
    scope_ids: Vec<SemanticId>,
    scope_paths: Vec<String>,
    next_scope: usize,
    namespace: SemanticId,
    owner: SemanticId,
    resolutions: Vec<NameResolution>,
}

#[derive(Clone)]
struct ResolvedBinding {
    id: String,
    ty: BodyType,
    binding: Option<SemanticId>,
    namespace: Option<SemanticId>,
    scope: Option<SemanticId>,
    path: Vec<String>,
    provenance: Option<ResolutionProvenance>,
}

impl ResolvedBinding {
    fn plain(id: String, ty: BodyType) -> Self {
        Self {
            id,
            ty,
            binding: None,
            namespace: None,
            scope: None,
            path: Vec::new(),
            provenance: None,
        }
    }
}

/// Lexical role of a bound name; recorded so tools can distinguish parameter,
/// local binding, carried iteration-state, and traversal-index declarations.
#[derive(Debug, Clone, Copy)]
enum BoundNameKind {
    Parameter,
    Binding,
    IterationState,
    /// The index binding of an enclosing bounded sequence traversal; element
    /// projections through it are discharged by traversal semantics.
    TraversalIndex,
}

impl BoundNameKind {
    fn resolved_kind(self) -> ResolvedNameKind {
        match self {
            Self::Parameter => ResolvedNameKind::Parameter,
            Self::Binding => ResolvedNameKind::Binding,
            Self::IterationState | Self::TraversalIndex => ResolvedNameKind::IterationState,
        }
    }

    fn binding_kind(self) -> &'static str {
        match self {
            Self::Parameter => "parameter",
            Self::Binding => "local",
            Self::IterationState => "iteration-state",
            Self::TraversalIndex => "iteration-index",
        }
    }
}

impl BindingEnv {
    fn new(module: &str, function: &str) -> Self {
        let namespace = module_id(module);
        let owner = function_id(module, function);
        let root_scope = scope_id(&namespace, &owner, "body");
        Self {
            scopes: vec![std::collections::BTreeMap::new()],
            scope_ids: vec![root_scope],
            scope_paths: vec!["body".to_owned()],
            next_scope: 0,
            namespace,
            owner,
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
        let scope_index = self.scopes.len() - 1;
        let scope = self
            .scopes
            .get_mut(scope_index)
            .expect("scope stack is non-empty");
        if let Some((_, existing, _)) = scope.get(&name) {
            let _ = existing;
            diagnostics.push(elaboration_diagnostic(
                "MNE110",
                "binding is ambiguous in this lexical scope",
                declaration,
            ));
            return;
        }
        let slot = scope.len();
        let scope_identity = self.scope_ids[scope_index].clone();
        let binding = binding_id(&scope_identity, kind.binding_kind(), slot);
        let resolution = NameResolution::new(declaration, declaration, kind.resolved_kind())
            .with_binding_metadata(
                binding.clone(),
                self.namespace.clone(),
                scope_identity.clone(),
                vec![name.clone()],
                ResolutionProvenance::Local,
                binding.clone(),
            );
        self.resolutions.push(resolution);
        scope.insert(
            name.clone(),
            (
                ResolvedBinding {
                    id,
                    ty,
                    binding: Some(binding),
                    namespace: Some(self.namespace.clone()),
                    scope: Some(scope_identity),
                    path: vec![name],
                    provenance: Some(ResolutionProvenance::Local),
                },
                declaration,
                kind,
            ),
        );
    }

    fn resolve(
        &mut self,
        name: &str,
        span: SourceSpan,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        self.resolve_with_kind(name, span, diagnostics)
            .map(|(binding, _kind)| binding)
    }

    /// Resolve a name and report its lexical role so indexing can recognize
    /// traversal-domain bindings.
    fn resolve_with_kind(
        &mut self,
        name: &str,
        span: SourceSpan,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<(ResolvedBinding, BoundNameKind)> {
        for scope in self.scopes.iter().rev() {
            if let Some((binding, declaration, kind)) = scope.get(name) {
                self.resolutions.push(
                    NameResolution::new(span, *declaration, kind.resolved_kind())
                        .with_binding_metadata(
                            binding.binding.clone().expect("lexical binding identity"),
                            binding
                                .namespace
                                .clone()
                                .expect("lexical namespace identity"),
                            binding.scope.clone().expect("lexical scope identity"),
                            binding.path.clone(),
                            binding
                                .provenance
                                .clone()
                                .expect("lexical binding provenance"),
                            binding
                                .binding
                                .clone()
                                .expect("lexical declaration identity"),
                        ),
                );
                return Some((binding.clone(), *kind));
            }
        }
        diagnostics.push(elaboration_diagnostic(
            "MNE102",
            "name does not resolve to a unique binding in the current lexical environment",
            span,
        ));
        None
    }

    /// Whether `name` is currently bound as a bounded-traversal index.
    fn is_traversal_index(&self, name: &str) -> bool {
        self.scopes
            .iter()
            .rev()
            .any(|scope| matches!(scope.get(name), Some((_, _, BoundNameKind::TraversalIndex))))
    }

    fn binds(&self, name: &str) -> bool {
        self.scopes
            .iter()
            .rev()
            .any(|scope| scope.contains_key(name))
    }

    fn push(&mut self) {
        let parent_path = self
            .scope_paths
            .last()
            .expect("scope path stack is non-empty")
            .clone();
        let path = format!("{parent_path}/block:{}", self.next_scope);
        self.next_scope += 1;
        self.scopes.push(std::collections::BTreeMap::new());
        self.scope_ids
            .push(scope_id(&self.namespace, &self.owner, &path));
        self.scope_paths.push(path);
    }

    fn pop(&mut self) {
        self.scopes.pop();
        self.scope_ids.pop();
        self.scope_paths.pop();
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
    namespace: String,
    owner: SemanticId,
    finite_types: &'a BTreeMap<String, FiniteType>,
    record_types: &'a BTreeMap<String, RecordType>,
    signatures: &'a BTreeMap<String, FunctionSignature>,
    declarations: &'a DeclarationSpans,
    namespace_aliases: &'a BTreeMap<String, String>,
    direct_imports: &'a BTreeSet<String>,
    generic_map: BTreeMap<String, mncs_model::GenericParamKind>,
    resolutions: Vec<NameResolution>,
    in_iteration: bool,
}

impl<'a> BodyBuilder<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        output_type: BodyType,
        function: String,
        namespace: String,
        finite_types: &'a BTreeMap<String, FiniteType>,
        record_types: &'a BTreeMap<String, RecordType>,
        signatures: &'a BTreeMap<String, FunctionSignature>,
        declarations: &'a DeclarationSpans,
        namespace_aliases: &'a BTreeMap<String, String>,
        direct_imports: &'a BTreeSet<String>,
        generic_map: BTreeMap<String, mncs_model::GenericParamKind>,
    ) -> Self {
        let owner = function_id(&namespace, &function);
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
            namespace,
            owner,
            finite_types,
            record_types,
            signatures,
            declarations,
            namespace_aliases,
            direct_imports,
            generic_map,
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

    fn body_scope(&self) -> SemanticId {
        let namespace = module_id(&self.namespace);
        scope_id(&namespace, &self.owner, "body")
    }

    #[allow(clippy::too_many_arguments)]
    fn semantic_resolution(
        &self,
        occurrence: SourceSpan,
        declaration: SourceSpan,
        kind: ResolvedNameKind,
        binding_kind: &str,
        spelling: &str,
        declaration_namespace: &str,
        declaration_identity: &SemanticId,
    ) -> NameResolution {
        let namespace = module_id(declaration_namespace);
        let module_scope = scope_id(&namespace, &namespace, "module");
        let binding = binding_id_for(&module_scope, binding_kind, declaration_identity);
        let scope = self.body_scope();
        let mut path = spelling.split('.').map(str::to_owned).collect::<Vec<_>>();
        if path.is_empty() {
            path.push(spelling.to_owned());
        }
        if declaration_namespace != self.namespace
            && !path.contains(&declaration_namespace.to_owned())
        {
            path.push(declaration_namespace.to_owned());
        }
        path.push(declaration_identity.0.clone());
        let provenance = if declaration_namespace == self.namespace {
            ResolutionProvenance::Local
        } else if path
            .first()
            .is_some_and(|route| self.namespace_aliases.contains_key(route))
        {
            ResolutionProvenance::Aliased
        } else if spelling.contains('.') {
            ResolutionProvenance::Qualified
        } else if self.direct_imports.contains(declaration_namespace) {
            ResolutionProvenance::DirectImport
        } else {
            ResolutionProvenance::TransitiveImport
        };
        NameResolution::new(occurrence, declaration, kind).with_binding_metadata(
            binding,
            module_id(&self.namespace),
            scope,
            path,
            provenance,
            declaration_identity.clone(),
        )
    }

    fn module_resolution(
        &self,
        occurrence: SourceSpan,
        declaration: SourceSpan,
        kind: ResolvedNameKind,
        binding_kind: &str,
        spelling: &str,
        declaration_identity: &SemanticId,
    ) -> NameResolution {
        let declaration_namespace = semantic_namespace_from_identity(declaration_identity);
        self.semantic_resolution(
            occurrence,
            declaration,
            kind,
            binding_kind,
            spelling,
            &declaration_namespace,
            declaration_identity,
        )
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
                let declared = profile_type_with_generics(
                    &value_type.text,
                    value_type.span,
                    self.finite_types,
                    self.record_types,
                    &self.generic_map,
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
            over_source,
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
        // Bounded sequence traversal (Profile 0.7): the step count derives
        // from the traversed sequence's declared bound rather than a source
        // literal.
        let traversal = over_source.as_deref().map(|source| {
            let resolved = self.elaborate_expr(source, None, env, diagnostics);
            (source.span(), resolved)
        });
        let mut traversal_length = None;
        let mut traversal_element: Option<Box<BodyType>> = None;
        let bound_u32 = match &traversal {
            Some((source_span, resolved)) => {
                let Some(resolved) = resolved else {
                    return;
                };
                let BodyType::Sequence { element, bound } = &resolved.ty else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE180",
                        "bounded traversal requires a bounded-sequence subject",
                        *source_span,
                    ));
                    return;
                };
                traversal_element = Some(element.clone());
                let ceiling = bound.ceiling();
                if ceiling > mncs_model::MAX_SEQUENCE_BOUND {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE182",
                        format!(
                            "sequence traversal bound must not exceed the profile ceiling {}",
                            mncs_model::MAX_SEQUENCE_BOUND
                        ),
                        *source_span,
                    ));
                    return;
                }
                // The runtime length observation drives both exact sequences
                // (constant-foldable) and views (runtime length).
                let length_id = self.new_value("iteration_domain_len");
                self.blocks[self.current].operations.push(BodyOperation {
                    id: length_id.clone(),
                    kind: BodyOperationKind::SequenceLength {
                        bound: bound.clone(),
                    },
                    operands: vec![resolved.id.clone()],
                    results: vec![BodyValue {
                        id: length_id.clone(),
                        ty: BodyType::Integer(IntegerType {
                            bits: 64,
                            signed: false,
                        }),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                });
                traversal_length = Some((resolved.id.clone(), length_id));
                ceiling
            }
            None => {
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
                bound_u32
            }
        };
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
        let carried_type = profile_type_with_generics(
            &state_type.text,
            state_type.span,
            self.finite_types,
            self.record_types,
            &self.generic_map,
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
        let bound_id = if let Some((_, length_id)) = &traversal_length {
            // Sequence traversal counts down from the observed runtime length;
            // exact sequences fold this observation to their declared length.
            length_id.clone()
        } else {
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
            bound_id
        };
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
        // Bounded traversal binds the loop index in the body scope. The
        // index is derived from the countdown counter so the traversal
        // domain fact (0 .. len) is machine knowledge, not a runtime check.
        if let Some((_, length_id)) = &traversal_length {
            let index_id = self.new_value("iteration_index");
            self.blocks[self.current].operations.push(BodyOperation {
                id: index_id.clone(),
                kind: BodyOperationKind::Integer {
                    operator: "sub".to_owned(),
                    operand_type: IntegerType {
                        bits: 64,
                        signed: false,
                    },
                    intent: ArithmeticIntent::Wrapping,
                },
                operands: vec![length_id.clone(), header_counter.clone()],
                results: vec![BodyValue {
                    id: index_id.clone(),
                    ty: BodyType::Integer(IntegerType {
                        bits: 64,
                        signed: false,
                    }),
                }],
                contracts: Vec::new(),
                assumptions: Vec::new(),
                machine_intent: None,
                lowering: None,
                portability: None,
            });
            env.bind(
                name.text.clone(),
                index_id,
                BodyType::Integer(IntegerType {
                    bits: 64,
                    signed: false,
                }),
                name.span,
                BoundNameKind::TraversalIndex,
                diagnostics,
            );
        }
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
            domain: match (&traversal, &traversal_element) {
                (Some(_), Some(element)) => IterationDomain::OverSequence {
                    element_type: element.clone(),
                },
                _ => IterationDomain::Attempts,
            },
            sequence_bound: traversal.as_ref().and_then(|(_, resolved)| {
                resolved.as_ref().and_then(|resolved| match &resolved.ty {
                    BodyType::Sequence { bound, .. } => Some(bound.clone()),
                    _ => None,
                })
            }),
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
            AstExpr::QualifiedPath { segments, span } => {
                if segments.len() < 3 {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE183",
                        "qualified path is not a value; qualify a callable or nominal declaration",
                        *span,
                    ));
                    return None;
                }
                if env.binds(&segments[0].text) {
                    let mut projected = AstExpr::Name(segments[0].clone());
                    for field in &segments[1..] {
                        projected = AstExpr::FieldProject {
                            base: Box::new(projected),
                            field: field.clone(),
                            span: *span,
                        };
                    }
                    return self.elaborate_expr(&projected, expected, env, diagnostics);
                }
                let type_name = SpannedText {
                    text: segments[..segments.len() - 1]
                        .iter()
                        .map(|segment| segment.text.as_str())
                        .collect::<Vec<_>>()
                        .join("."),
                    span: SourceSpan {
                        start: segments.first()?.span.start,
                        end: segments[segments.len() - 2].span.end,
                        line: segments.first()?.span.line,
                        column: segments.first()?.span.column,
                    },
                };
                let variant = segments.last()?.clone();
                self.elaborate_expr(
                    &AstExpr::FiniteVariant {
                        type_name,
                        variant,
                        fields: Vec::new(),
                        span: *span,
                    },
                    expected,
                    env,
                    diagnostics,
                )
            }
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
                    // Byte-typed literals adapt to the unsigned 8-bit domain.
                    Some(BodyType::Byte) if (0..=255).contains(value) => BodyType::Byte,
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
                Some(ResolvedBinding::plain(id, ty))
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
                Some(ResolvedBinding::plain(id, ty))
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
                if let Some(&declaration) = self
                    .declarations
                    .qualified_finite_types
                    .get(&type_name.text)
                    .or_else(|| self.declarations.finite_types.get(&type_name.text))
                {
                    self.resolutions.push(self.module_resolution(
                        type_name.span,
                        declaration,
                        ResolvedNameKind::FiniteType,
                        "finite-type",
                        &type_name.text,
                        &finite_type.identity,
                    ));
                }
                if let Some(&declaration) = self
                    .declarations
                    .qualified_finite_variants
                    .get(&(type_name.text.clone(), variant.text.clone()))
                    .or_else(|| {
                        self.declarations
                            .finite_variants
                            .get(&(type_name.text.clone(), variant.text.clone()))
                    })
                {
                    self.resolutions.push(self.module_resolution(
                        variant.span,
                        declaration,
                        ResolvedNameKind::FiniteVariant,
                        "finite-variant",
                        &format!("{}.{}", type_name.text, variant.text),
                        &declared_variant.identity,
                    ));
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
                Some(ResolvedBinding::plain(id, ty))
            }
            AstExpr::Call {
                function,
                generic_args,
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
                if let Some(&declaration) = self
                    .declarations
                    .qualified_functions
                    .get(&function.text)
                    .or_else(|| self.declarations.functions.get(&function.text))
                {
                    let resolution = self.semantic_resolution(
                        function.span,
                        declaration,
                        ResolvedNameKind::Function,
                        "function",
                        &function.text,
                        &signature.namespace,
                        &signature.identity,
                    );
                    self.resolutions.push(resolution);
                }
                // ---- Generic argument elaboration (Profile 0.10) ----
                let elaborated_generic_args: Vec<mncs_model::GenericArg> = {
                    let callee_params = &signature.generic_params;
                    if callee_params.is_empty() {
                        if !generic_args.is_empty() {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE222",
                                "generic arguments supplied for non-generic function",
                                function.span,
                            ));
                            return None;
                        }
                        Vec::new()
                    } else {
                        if generic_args.is_empty() {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE220",
                                format!(
                                    "generic function '{}' requires {} generic argument(s); inference is not available in this tranche",
                                    function.text,
                                    callee_params.len()
                                ),
                                function.span,
                            ));
                            return None;
                        }
                        if generic_args.len() != callee_params.len() {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE221",
                                format!(
                                    "generic argument count mismatch for '{}': expected {}, got {}",
                                    function.text,
                                    callee_params.len(),
                                    generic_args.len()
                                ),
                                function.span,
                            ));
                            return None;
                        }
                        let mut args_out = Vec::new();
                        for (param, arg) in callee_params.iter().zip(generic_args) {
                            match param.kind {
                                mncs_model::GenericParamKind::Type => {
                                    let ty = profile_type_with_generics(
                                        &arg.text.text,
                                        arg.text.span,
                                        self.finite_types,
                                        self.record_types,
                                        &self.generic_map,
                                        diagnostics,
                                    );
                                    // Reject Nat value being passed as Type
                                    if arg.text.text.parse::<u32>().is_ok() {
                                        // If arg text parses as integer but param expects Type, it's wrong kind
                                        diagnostics.push(elaboration_diagnostic(
                                            "MNE222",
                                            format!(
                                                "generic type parameter '{}' received value argument '{}'",
                                                param.name, arg.text.text
                                            ),
                                            arg.text.span,
                                        ));
                                        return None;
                                    }
                                    // If the parsed type is still a generic value param misuse, it will be caught as Named vs GenericParam
                                    args_out.push(mncs_model::GenericArg::Type { ty });
                                }
                                mncs_model::GenericParamKind::Nat => {
                                    let text = arg.text.text.trim();
                                    // Try integer literal
                                    if let Ok(val) = text.parse::<u32>() {
                                        if val > mncs_model::MAX_SEQUENCE_BOUND {
                                            diagnostics.push(elaboration_diagnostic(
                                                "MNE225",
                                                format!(
                                                    "sequence bound value {val} exceeds profile ceiling {}",
                                                    mncs_model::MAX_SEQUENCE_BOUND
                                                ),
                                                arg.text.span,
                                            ));
                                            return None;
                                        }
                                        args_out.push(mncs_model::GenericArg::Value { value: val });
                                    } else if let Some(kind) = self.generic_map.get(text) {
                                        if *kind == mncs_model::GenericParamKind::Nat {
                                            args_out.push(mncs_model::GenericArg::ValueParam {
                                                name: text.to_owned(),
                                            });
                                        } else {
                                            diagnostics.push(elaboration_diagnostic(
                                                "MNE222",
                                                format!(
                                                    "generic value parameter '{}' received type argument '{}'",
                                                    param.name, text
                                                ),
                                                arg.text.span,
                                            ));
                                            return None;
                                        }
                                    } else if profile_type_supported_for_generic_kind(
                                        text,
                                        self.finite_types,
                                        self.record_types,
                                    ) {
                                        diagnostics.push(elaboration_diagnostic(
                                            "MNE222",
                                            format!(
                                                "generic value parameter '{}' received type argument '{}'",
                                                param.name, text
                                            ),
                                            arg.text.span,
                                        ));
                                        return None;
                                    } else if text.chars().all(|c| c.is_ascii_digit()) {
                                        diagnostics.push(elaboration_diagnostic(
                                            "MNE224",
                                            format!("value argument '{text}' is not a valid Nat literal"),
                                            arg.text.span,
                                        ));
                                        return None;
                                    } else {
                                        diagnostics.push(elaboration_diagnostic(
                                            "MNE224",
                                            format!(
                                                "value argument for '{}' must be a Nat literal or Nat parameter, got '{text}'",
                                                param.name
                                            ),
                                            arg.text.span,
                                        ));
                                        return None;
                                    }
                                }
                            }
                        }
                        args_out
                    }
                };
                // Compute concrete callee signature after substituting generic args (if any)
                let (concrete_inputs, concrete_output) = if elaborated_generic_args.is_empty() {
                    (signature.inputs.clone(), signature.output.clone())
                } else {
                    let mut type_map = std::collections::BTreeMap::new();
                    let mut value_map = std::collections::BTreeMap::new();
                    for (param, arg) in signature
                        .generic_params
                        .iter()
                        .zip(&elaborated_generic_args)
                    {
                        match (param.kind, arg) {
                            (
                                mncs_model::GenericParamKind::Type,
                                mncs_model::GenericArg::Type { ty },
                            ) => {
                                type_map.insert(param.name.clone(), ty.clone());
                            }
                            (
                                mncs_model::GenericParamKind::Nat,
                                mncs_model::GenericArg::Value { value },
                            ) => {
                                value_map.insert(param.name.clone(), *value);
                            }
                            (
                                mncs_model::GenericParamKind::Nat,
                                mncs_model::GenericArg::ValueParam { name: _ },
                            ) => {
                                // Forwarding: keep as param reference for now, but we need to preserve the reference
                                // Instead we store a placeholder that will be resolved during specialization.
                                // For this tranche we treat forwarding as not yet concrete; keep ValueParam.
                                // The concrete inputs after substitution will still contain Param bound if forwarded.
                                // That's acceptable for body building because caller’s specialization will later resolve.
                                // For now we keep value_map entry as lookup to caller’s param value if caller is generic and will be specialized.
                                // If caller is non-generic, this should not happen because ValueParam would have been rejected as concrete.
                                // So we allow it and treat as symbolic.
                                // We encode symbolic as a special marker: we will keep bound as Param(name) via not inserting concrete value, instead we keep mapping from callee Nat param to caller's Nat param name via a separate structure.
                                // To avoid complexity, for now we handle forwarding by mapping callee Nat param to the caller’s Nat param's current substitution status: if caller’s generic_map contains that name, we treat it as ValueParam.
                                // So we will not insert into value_map, but we need to know that concrete_inputs substitution should replace callee bound Param(param.name) with caller’s Param via type_map/value_map forwarding.
                                // Simplify: create a symbolic entry that during substitution will look up caller’s param.
                                // For now, we insert a placeholder that indicates forwarding: we store the name as value_map entry with a sentinel that will be handled during type substitution.
                                // Easiest: treat ValueParam as a distinct GenericArg variant that during substitution will be replaced by the caller’s param value if that caller’s param is later substituted.
                                // So we keep value_map empty for these, but we need to propagate the Param name through type substitution.
                                // For sequence bound Param handling, type_map/value_map will need to map callee’s Nat param name to caller’s Nat param name.
                                // We can achieve by storing value_map entry as the caller's param name's current bound? But we need indirection.
                                // Simplify: Do not substitute now; keep elaborated_generic_args with ValueParam and let specialization resolve later.
                                // So we skip concrete substitution for these and keep signature types generic; the call's operand type checking will be deferred to specialization.
                                // For now, we treat forwarding calls as not checking concrete input types strictly.
                                // So we set a flag to skip strict type checking when forwarding.
                            }
                            _ => {
                                diagnostics.push(elaboration_diagnostic(
                                    "MNE222",
                                    "generic argument kind mismatch",
                                    function.span,
                                ));
                                return None;
                            }
                        }
                    }
                    // If there was any forwarding (ValueParam), we cannot fully concretize inputs now.
                    let has_forwarding = elaborated_generic_args.iter().any(|a| match a {
                        mncs_model::GenericArg::ValueParam { .. } => true,
                        mncs_model::GenericArg::Type { ty } => {
                            matches!(ty, mncs_model::BodyType::GenericParam { .. })
                        }
                        _ => false,
                    });
                    if has_forwarding {
                        // Keep signature generic inputs as is; validation will be performed after specialization when caller is specialized.
                        (signature.inputs.clone(), signature.output.clone())
                    } else {
                        // Fully concrete substitution
                        let substituted_inputs: Vec<mncs_model::BodyType> = signature
                            .inputs
                            .iter()
                            .map(|ty| substitute_body_type(ty.clone(), &type_map, &value_map))
                            .collect();
                        let substituted_output =
                            substitute_body_type(signature.output.clone(), &type_map, &value_map);
                        (substituted_inputs, substituted_output)
                    }
                };
                if arguments.len() != concrete_inputs.len() {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE132",
                        "call argument arity does not match the callee signature",
                        *span,
                    ));
                    return None;
                }
                let mut operands = Vec::new();
                for (source_argument, parameter_type) in arguments.iter().zip(&concrete_inputs) {
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
                if expected.is_some_and(|expected| expected != &concrete_output) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE135",
                        "call result does not have the required expression type",
                        *span,
                    ));
                }
                let id = self.new_value("call");
                // Compute instantiation identity for concrete substitutions; forwarding keeps symbolic
                let instantiation = if elaborated_generic_args.is_empty()
                    || elaborated_generic_args.iter().any(|a| !a.is_concrete())
                {
                    None
                } else {
                    let canonical = elaborated_generic_args
                        .iter()
                        .map(|a| a.canonical_string())
                        .collect::<Vec<_>>()
                        .join("|");
                    let hash = mncs_model::sha256_hex(canonical.as_bytes());
                    Some(mncs_model::instantiation_id(&signature.identity, &hash))
                };
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind: BodyOperationKind::Call {
                        function: signature.identity.clone(),
                        function_name: function.text.clone(),
                        required_capabilities: signature.capabilities.clone(),
                        effects: signature.effects.clone(),
                        generic_args: elaborated_generic_args.clone(),
                        instantiation: instantiation.clone(),
                        specialization: None,
                    },
                    operands,
                    results: vec![BodyValue {
                        id: id.clone(),
                        ty: concrete_output.clone(),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                });
                Some(ResolvedBinding::plain(id, concrete_output.clone()))
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
                    .values()
                    .find(|candidate| &candidate.identity == type_identity)
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
                        if self
                            .finite_types
                            .get(&qualifier.text)
                            .map(|candidate| &candidate.identity)
                            != Some(type_identity)
                        {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE176",
                                "qualified pattern names a different finite type than the match subject",
                                qualifier.span,
                            ));
                            continue;
                        }
                        if let Some(&declaration) = self
                            .declarations
                            .qualified_finite_types
                            .get(&qualifier.text)
                            .or_else(|| self.declarations.finite_types.get(&qualifier.text))
                        {
                            let resolution = NameResolution::new(
                                qualifier.span,
                                declaration,
                                ResolvedNameKind::FiniteType,
                            )
                            .with_binding_metadata(
                                binding_id_for(
                                    &scope_id(
                                        &module_id(&self.namespace),
                                        &module_id(&self.namespace),
                                        "module",
                                    ),
                                    "finite-type",
                                    type_identity,
                                ),
                                module_id(&self.namespace),
                                self.body_scope(),
                                vec![qualifier.text.clone(), type_identity.0.clone()],
                                if qualifier.text.contains('.') {
                                    ResolutionProvenance::Qualified
                                } else {
                                    ResolutionProvenance::Local
                                },
                                type_identity.clone(),
                            );
                            self.resolutions.push(resolution);
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
                    let qualified_declaration = arm.type_name.as_ref().and_then(|qualifier| {
                        self.declarations
                            .qualified_finite_variants
                            .get(&(qualifier.text.clone(), arm.variant.text.clone()))
                            .copied()
                    });
                    if let Some(declaration) = qualified_declaration.or_else(|| {
                        self.declarations
                            .finite_variants
                            .get(&(type_name.clone(), arm.variant.text.clone()))
                            .copied()
                    }) {
                        let resolution = NameResolution::new(
                            arm.variant.span,
                            declaration,
                            ResolvedNameKind::FiniteVariant,
                        )
                        .with_binding_metadata(
                            binding_id_for(
                                &scope_id(
                                    &module_id(&self.namespace),
                                    &module_id(&self.namespace),
                                    "module",
                                ),
                                "finite-variant",
                                &variant.identity,
                            ),
                            module_id(&self.namespace),
                            self.body_scope(),
                            vec![arm.variant.text.clone(), variant.identity.0.clone()],
                            if arm.type_name.is_some() {
                                ResolutionProvenance::Qualified
                            } else {
                                ResolutionProvenance::Local
                            },
                            variant.identity.clone(),
                        );
                        self.resolutions.push(resolution);
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
                Some(ResolvedBinding::plain(result_id, result_type))
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
                if let Some(&declaration) = self
                    .declarations
                    .qualified_record_types
                    .get(&type_name.text)
                    .or_else(|| self.declarations.record_types.get(&type_name.text))
                {
                    let resolution = self.module_resolution(
                        type_name.span,
                        declaration,
                        ResolvedNameKind::RecordType,
                        "record-type",
                        &type_name.text,
                        &record_type.identity,
                    );
                    self.resolutions.push(resolution);
                }
                for (name, _) in fields {
                    if record_type
                        .fields
                        .iter()
                        .any(|field| field.name == name.text)
                    {
                        if let Some(&declaration) = self
                            .declarations
                            .qualified_record_fields
                            .get(&(type_name.text.clone(), name.text.clone()))
                            .or_else(|| {
                                self.declarations
                                    .record_fields
                                    .get(&(type_name.text.clone(), name.text.clone()))
                            })
                        {
                            let record_namespace =
                                semantic_namespace_from_identity(&record_type.identity);
                            let field_identity = mncs_model::record_field_id(
                                &record_namespace,
                                &record_type.name,
                                &name.text,
                            );
                            let resolution = self.module_resolution(
                                name.span,
                                declaration,
                                ResolvedNameKind::RecordField,
                                "record-field",
                                &format!("{}.{}", type_name.text, name.text),
                                &field_identity,
                            );
                            self.resolutions.push(resolution);
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
                Some(ResolvedBinding::plain(id, ty))
            }
            AstExpr::FieldProject { base, field, span } => {
                // Length observation on a bounded sequence: `xs.len` (0.7).
                // Field projection on sequences is reserved for `.len`.
                if field.text == "len" {
                    let subject = self.elaborate_expr(base, None, env, diagnostics)?;
                    if let BodyType::Sequence { bound, .. } = &subject.ty {
                        let id = self.new_value("seqlen");
                        self.blocks[self.current].operations.push(BodyOperation {
                            id: id.clone(),
                            kind: match &subject.ty {
                                BodyType::Sequence { bound, .. } => {
                                    BodyOperationKind::SequenceLength {
                                        bound: bound.clone(),
                                    }
                                }
                                _ => BodyOperationKind::SequenceLength {
                                    bound: mncs_model::SequenceBound::Exact(0),
                                },
                            },
                            operands: vec![subject.id.clone()],
                            results: vec![BodyValue {
                                id: id.clone(),
                                ty: BodyType::Integer(IntegerType {
                                    bits: 64,
                                    signed: false,
                                }),
                            }],
                            contracts: Vec::new(),
                            assumptions: Vec::new(),
                            machine_intent: None,
                            lowering: None,
                            portability: None,
                        });
                        if expected.is_some_and(|expected| {
                            *expected
                                != BodyType::Integer(IntegerType {
                                    bits: 64,
                                    signed: false,
                                })
                        }) {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE163",
                                "length observation does not have the required expression type",
                                *span,
                            ));
                        }
                        let _ = bound;
                        return Some(ResolvedBinding::plain(
                            id,
                            BodyType::Integer(IntegerType {
                                bits: 64,
                                signed: false,
                            }),
                        ));
                    }
                }
                let subject = self.elaborate_expr(base, None, env, diagnostics)?;
                let BodyType::Record {
                    identity: type_identity,
                    name: type_name,
                } = &subject.ty
                else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE161",
                        "field projection requires a value of a declared record type",
                        base.span(),
                    ));
                    return None;
                };
                let Some(record_type) = self
                    .record_types
                    .values()
                    .find(|candidate| &candidate.identity == type_identity)
                    .cloned()
                    .or_else(|| self.record_types.get(type_name).cloned())
                else {
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
                let qualified_declaration = self
                    .declarations
                    .qualified_record_fields
                    .iter()
                    .find(|((qualified_type, field_name), _)| {
                        field_name == &field.text
                            && qualified_type
                                .rsplit('.')
                                .next()
                                .is_some_and(|name| name == record_type.name)
                    })
                    .map(|(_, declaration)| declaration);
                if let Some(&declaration) = qualified_declaration.or_else(|| {
                    self.declarations
                        .record_fields
                        .get(&(record_type.name.clone(), field.text.clone()))
                }) {
                    let record_namespace = semantic_namespace_from_identity(&record_type.identity);
                    let field_identity = mncs_model::record_field_id(
                        &record_namespace,
                        &record_type.name,
                        &field.text,
                    );
                    let resolution = self.module_resolution(
                        field.span,
                        declaration,
                        ResolvedNameKind::RecordField,
                        "record-field",
                        &format!("{}.{}", record_type.name, field.text),
                        &field_identity,
                    );
                    self.resolutions.push(resolution);
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
            AstExpr::Select {
                condition,
                when_true,
                when_false,
                span: _,
            } => {
                let condition_binding = self.elaborate_expr(condition, None, env, diagnostics)?;
                // The operand type is the expected type when known; otherwise
                // the first candidate establishes it and the second must
                // agree. Both candidates are values of one selection.
                let (operand_type, true_binding) = match expected {
                    Some(candidate) => {
                        let binding =
                            self.elaborate_expr(when_true, Some(candidate), env, diagnostics)?;
                        (candidate.clone(), binding)
                    }
                    None => {
                        let binding = self.elaborate_expr(when_true, None, env, diagnostics)?;
                        (binding.ty.clone(), binding)
                    }
                };
                if true_binding.ty != operand_type {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE196",
                        "selection candidates must have the same type",
                        when_true.span(),
                    ));
                    return None;
                }
                let false_binding =
                    self.elaborate_expr(when_false, Some(&operand_type), env, diagnostics)?;
                if false_binding.ty != operand_type {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE196",
                        "selection candidates must have the same type",
                        when_false.span(),
                    ));
                    return None;
                }
                let required_condition = match &operand_type {
                    BodyType::Vector { lanes, .. } => BodyType::Mask { lanes: *lanes },
                    _ => BodyType::Named("bool".to_owned()),
                };
                if condition_binding.ty != required_condition {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE195",
                        "selection condition must be bool, or a lane-matched mask for vector selection",
                        condition.span(),
                    ));
                    return None;
                }
                let id = self.new_value("sel");
                let requirement_identity = SemanticId(format!(
                    "mncs:0.8:requirement:realization-branchless:{}:{}",
                    self.function, id
                ));
                let machine_intent = MachineIntentSpec {
                    intent: Intent {
                        identity: SemanticId(format!(
                            "mncs:0.8:intent:semantic-selection:{}:{}",
                            self.function, id
                        )),
                        statement: "pure conditional selection over two candidate values; \
                                    no control-flow divergence between them is part of the meaning"
                            .to_owned(),
                    },
                    preferences: vec![MachinePreference {
                        identity: SemanticId("mncs:0.8:preference:select-not-branch".to_owned()),
                        statement: "prefer a backend-native select/predicated realization over \
                                    branch lowering where the target can provide it"
                            .to_owned(),
                    }],
                    facts: Vec::new(),
                    requirements: vec![Requirement {
                        identity: requirement_identity.clone(),
                        subject: SemanticId(id.clone()),
                        statement: "realization must preserve selection semantics without \
                                    introducing data-dependent branch divergence between the candidates"
                            .to_owned(),
                    }],
                    obligations: Vec::new(),
                };
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind: BodyOperationKind::Select {
                        operand_type: Box::new(operand_type.clone()),
                    },
                    operands: vec![condition_binding.id, true_binding.id, false_binding.id],
                    results: vec![BodyValue {
                        id: id.clone(),
                        ty: operand_type.clone(),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: Some(machine_intent),
                    lowering: None,
                    portability: None,
                });
                Some(ResolvedBinding::plain(id, operand_type))
            }
            AstExpr::SequenceReplace {
                sequence,
                index,
                element,
                span,
            } => {
                let source = self.elaborate_expr(sequence, None, env, diagnostics)?;
                let BodyType::Sequence {
                    element: element_type,
                    bound,
                } = source.ty.clone()
                else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE197",
                        "functional update requires a bounded-sequence value",
                        sequence.span(),
                    ));
                    return None;
                };
                if matches!(bound, mncs_model::SequenceBound::UpTo(_)) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE198",
                        "functional update requires an exact-bound sequence this tranche; views refuse",
                        *span,
                    ));
                    return None;
                }
                let counter_type = BodyType::Integer(IntegerType {
                    bits: 64,
                    signed: false,
                });
                let index_binding =
                    self.elaborate_expr(index, Some(&counter_type), env, diagnostics)?;
                if index_binding.ty != counter_type {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE199",
                        "functional update index must have u64 type",
                        index.span(),
                    ));
                    return None;
                }
                let element_binding =
                    self.elaborate_expr(element, Some(&element_type), env, diagnostics)?;
                if element_binding.ty != *element_type {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE200",
                        "functional update element does not match the declared element type",
                        element.span(),
                    ));
                    return None;
                }
                // A literal index at or beyond an exact declared bound is
                // provably invalid: fail closed here rather than deferring a
                // guaranteed runtime failure.
                if let (mncs_model::SequenceBound::Exact(length), AstExpr::Integer { value, .. }) =
                    (&bound, index.as_ref())
                {
                    if *value < 0 || *value >= i128::from(*length) {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE192",
                            format!(
                                "index {value} is outside the statically known domain 0..{length}"
                            ),
                            index.span(),
                        ));
                        return None;
                    }
                }
                // Index validity evidence mirrors projection: literal indices
                // inside an exact bound are static; traversal indices are
                // discharged by traversal semantics; anything else keeps an
                // explicit runtime-checked failure obligation.
                let evidence = match index.as_ref() {
                    AstExpr::Integer { value, .. }
                        if matches!(&bound, mncs_model::SequenceBound::Exact(length)
                            if *value >= 0 && (*value as u64) < u64::from(*length)) =>
                    {
                        mncs_model::BoundsEvidence::StaticExact
                    }
                    _ if matches!(index.as_ref(), AstExpr::Name(name) if env.is_traversal_index(&name.text)) => {
                        mncs_model::BoundsEvidence::TraversalDomain
                    }
                    _ => mncs_model::BoundsEvidence::RuntimeChecked {
                        failure: FailureMode::Isolated,
                    },
                };
                let result_type = BodyType::Sequence {
                    element: element_type.clone(),
                    bound: bound.clone(),
                };
                let id = self.new_value("rep");
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind: BodyOperationKind::SequenceReplace {
                        element_type: element_type.clone(),
                        bound: bound.clone(),
                        evidence: evidence.clone(),
                    },
                    operands: vec![source.id, index_binding.id, element_binding.id],
                    results: vec![BodyValue {
                        id: id.clone(),
                        ty: result_type.clone(),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                });
                Some(ResolvedBinding::plain(id, result_type))
            }
            AstExpr::VectorIntrinsic {
                name,
                arguments,
                span,
            } => self.elaborate_vector_intrinsic(
                &name.text,
                arguments,
                *span,
                expected,
                env,
                diagnostics,
            ),
            AstExpr::SequenceLiteral { elements, span } => {
                let BodyType::Sequence {
                    element: element_type,
                    bound: mncs_model::SequenceBound::Exact(length),
                } = expected
                    .cloned()
                    .unwrap_or(BodyType::Named("invalid".to_owned()))
                else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE183",
                        "sequence literals require an exact bounded-sequence expected type",
                        *span,
                    ));
                    return None;
                };
                if elements.len() != length as usize {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE184",
                        format!(
                            "sequence literal supplies {0} elements but the declared exact length is {length}",
                            elements.len()
                        ),
                        *span,
                    ));
                    return None;
                }
                let mut operands = Vec::with_capacity(elements.len());
                for element in elements {
                    let resolved =
                        self.elaborate_expr(element, Some(&element_type), env, diagnostics)?;
                    if &resolved.ty != element_type.as_ref() {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE185",
                            "sequence element value does not match the declared element type",
                            element.span(),
                        ));
                    }
                    operands.push(resolved.id);
                }
                let id = self.new_value("seq");
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind: BodyOperationKind::SequenceConstruct {
                        element_type,
                        length,
                    },
                    operands,
                    results: vec![BodyValue {
                        id: id.clone(),
                        ty: expected
                            .cloned()
                            .unwrap_or_else(|| BodyType::Named("invalid".to_owned())),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                });
                Some(ResolvedBinding::plain(
                    id,
                    expected
                        .cloned()
                        .unwrap_or_else(|| BodyType::Named("invalid".to_owned())),
                ))
            }
            AstExpr::Index { base, index, span } => {
                let subject = self.elaborate_expr(base, None, env, diagnostics)?;
                let BodyType::Sequence {
                    element: element_type,
                    bound,
                } = &subject.ty
                else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE186",
                        "index observation requires a bounded-sequence value",
                        base.span(),
                    ));
                    return None;
                };
                let counter_type = BodyType::Integer(IntegerType {
                    bits: 64,
                    signed: false,
                });
                let index_value =
                    self.elaborate_expr(index, Some(&counter_type), env, diagnostics)?;
                // A literal index at or beyond an exact declared bound is
                // provably invalid: fail closed at elaboration instead of
                // deferring a guaranteed runtime failure.
                if let (mncs_model::SequenceBound::Exact(length), AstExpr::Integer { value, .. }) =
                    (&bound, index.as_ref())
                {
                    if *value < 0 || *value >= i128::from(*length) {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE192",
                            format!(
                                "index {value} is outside the statically known domain 0..{length}"
                            ),
                            index.span(),
                        ));
                        return None;
                    }
                }
                // Bounds evidence is first-class machine knowledge: a literal
                // index inside an exact declared bound is statically valid; a
                // traversal-domain binding is discharged by traversal
                // semantics; anything else keeps an explicit runtime check.
                let evidence = match (&bound, index.as_ref()) {
                    (mncs_model::SequenceBound::Exact(length), AstExpr::Integer { value, .. })
                        if *value >= 0 && (*value as u64) < u64::from(*length) =>
                    {
                        mncs_model::BoundsEvidence::StaticExact
                    }
                    _ if matches!(index.as_ref(), AstExpr::Name(name) if env.is_traversal_index(&name.text)) => {
                        mncs_model::BoundsEvidence::TraversalDomain
                    }
                    _ => mncs_model::BoundsEvidence::RuntimeChecked {
                        failure: FailureMode::Isolated,
                    },
                };
                if expected.is_some_and(|expected| expected != element_type.as_ref()) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE163",
                        "projected sequence element does not have the required expression type",
                        *span,
                    ));
                }
                let id = self.new_value("elem");
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind: BodyOperationKind::SequenceProject {
                        bound: bound.clone(),
                        evidence,
                    },
                    operands: vec![subject.id, index_value.id],
                    results: vec![BodyValue {
                        id: id.clone(),
                        ty: (**element_type).clone(),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                });
                Some(ResolvedBinding::plain(id, (**element_type).clone()))
            }
            AstExpr::Slice {
                base,
                start,
                end,
                span,
            } => {
                let subject = self.elaborate_expr(base, None, env, diagnostics)?;
                let BodyType::Sequence {
                    element: _,
                    bound: source_bound,
                } = &subject.ty
                else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE187",
                        "view derivation requires a bounded-sequence source",
                        base.span(),
                    ));
                    return None;
                };
                let view_cap = source_bound.ceiling();
                let counter_type = BodyType::Integer(IntegerType {
                    bits: 64,
                    signed: false,
                });
                let start_value =
                    self.elaborate_expr(start, Some(&counter_type), env, diagnostics)?;
                let end_value = self.elaborate_expr(end, Some(&counter_type), env, diagnostics)?;
                let result_ty = BodyType::Sequence {
                    element: Box::new(match &subject.ty {
                        BodyType::Sequence { element, .. } => (**element).clone(),
                        _ => BodyType::Named("invalid".to_owned()),
                    }),
                    bound: mncs_model::SequenceBound::UpTo(view_cap),
                };
                if expected.is_some_and(|expected| expected != &result_ty) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE188",
                        format!(
                            "derived view has type {} which does not satisfy the required type",
                            result_ty.semantic_name()
                        ),
                        *span,
                    ));
                }
                let id = self.new_value("view");
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind: BodyOperationKind::ViewConstruct {
                        source_bound: source_bound.clone(),
                        view_bound: mncs_model::SequenceBound::UpTo(view_cap),
                    },
                    operands: vec![subject.id, start_value.id, end_value.id],
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
                Some(ResolvedBinding::plain(id, result_ty))
            }
            AstExpr::Cast {
                value,
                target_type,
                span,
            } => {
                let subject = self.elaborate_expr(value, None, env, diagnostics)?;
                let to = profile_type(
                    &target_type.text,
                    target_type.span,
                    self.finite_types,
                    self.record_types,
                    diagnostics,
                );
                let convertible_source = |ty: &BodyType| {
                    matches!(ty, BodyType::Byte)
                        || matches!(ty, BodyType::Integer(integer) if matches!(integer.bits, 1..=64))
                        || matches!(ty, BodyType::Named(name) if name == "bool")
                };
                let convertible_target = |ty: &BodyType| {
                    matches!(ty, BodyType::Byte)
                        || matches!(ty, BodyType::Integer(integer) if matches!(integer.bits, 1..=64))
                };
                if !convertible_source(&subject.ty) || !convertible_target(&to) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE189",
                        "explicit conversions require scalar integer or byte types on both sides",
                        *span,
                    ));
                    return None;
                }
                if expected.is_some_and(|expected| expected != &to) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE163",
                        "converted value does not have the required expression type",
                        *span,
                    ));
                }
                let from = subject.ty.clone();
                let id = self.new_value("cast");
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind: BodyOperationKind::Convert {
                        from,
                        to: to.clone(),
                    },
                    operands: vec![subject.id],
                    results: vec![BodyValue {
                        id: id.clone(),
                        ty: to.clone(),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                });
                Some(ResolvedBinding::plain(id, to))
            }
            AstExpr::Binary {
                op, left, right, ..
            } => {
                // Shifts (Profile 0.7). A byte left operand shifts a byte
                // (count modulo 8); an integer left operand shifts within its
                // declared width (count modulo width). Both are total by
                // definition and carry wrapping intent.
                if matches!(op, AstBinaryOp::Shl | AstBinaryOp::Shr) {
                    let counter_type = BodyType::Integer(IntegerType {
                        bits: 64,
                        signed: false,
                    });
                    let left_value = self.elaborate_expr(left, None, env, diagnostics)?;
                    let count_value =
                        self.elaborate_expr(right, Some(&counter_type), env, diagnostics)?;
                    let operator = match op {
                        AstBinaryOp::Shl => "shl",
                        _ => "shr",
                    };
                    let id = self.new_value("sh");
                    let (kind, result_ty) = match &left_value.ty {
                        BodyType::Byte => (
                            BodyOperationKind::ByteShift {
                                operator: operator.to_owned(),
                            },
                            BodyType::Byte,
                        ),
                        BodyType::Integer(operand_type) => (
                            BodyOperationKind::Integer {
                                operator: operator.to_owned(),
                                operand_type: *operand_type,
                                intent: ArithmeticIntent::Wrapping,
                            },
                            left_value.ty.clone(),
                        ),
                        _ => {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE190",
                                "shift operands must shift a byte or integer value",
                                expr.span(),
                            ));
                            return None;
                        }
                    };
                    self.blocks[self.current].operations.push(BodyOperation {
                        id: id.clone(),
                        kind,
                        operands: vec![left_value.id, count_value.id],
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
                    return Some(ResolvedBinding::plain(id, result_ty));
                }
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
                    return Some(ResolvedBinding::plain(id, bool_type));
                }
                // Byte-oriented operators (Profile 0.7): bitwise and/or/xor
                // and unsigned comparisons over bytes.
                if left_value.ty == BodyType::Byte {
                    let is_compare = matches!(
                        op,
                        AstBinaryOp::Eq
                            | AstBinaryOp::Ne
                            | AstBinaryOp::Lt
                            | AstBinaryOp::Le
                            | AstBinaryOp::Gt
                            | AstBinaryOp::Ge
                    );
                    let is_bitwise = matches!(
                        op,
                        AstBinaryOp::BitwiseAnd | AstBinaryOp::BitwiseOr | AstBinaryOp::BitwiseXor
                    );
                    if !is_compare && !is_bitwise {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE191",
                            "bytes support bitwise operations, shifts, comparisons, and explicit conversions; use `as` for arithmetic",
                            expr.span(),
                        ));
                        return None;
                    }
                    let id = self.new_value("b");
                    let (kind, result_ty) = if is_compare {
                        (
                            BodyOperationKind::ByteCompare {
                                predicate: match op {
                                    AstBinaryOp::Eq => "eq",
                                    AstBinaryOp::Ne => "ne",
                                    AstBinaryOp::Lt => "lt",
                                    AstBinaryOp::Le => "le",
                                    AstBinaryOp::Gt => "gt",
                                    _ => "ge",
                                }
                                .to_owned(),
                            },
                            BodyType::Named("bool".to_owned()),
                        )
                    } else {
                        (
                            BodyOperationKind::ByteBitwise {
                                operator: match op {
                                    AstBinaryOp::BitwiseAnd => "and",
                                    AstBinaryOp::BitwiseOr => "or",
                                    _ => "xor",
                                }
                                .to_owned(),
                            },
                            BodyType::Byte,
                        )
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
                    return Some(ResolvedBinding::plain(id, result_ty));
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
                Some(ResolvedBinding::plain(id, result_ty))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn elaborate_vector_intrinsic(
        &mut self,
        name: &str,
        arguments: &[AstExpr],
        span: SourceSpan,
        expected: Option<&BodyType>,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        let arity = |wanted: usize, diagnostics: &mut Vec<SourceDiagnostic>| {
            if arguments.len() == wanted {
                true
            } else {
                diagnostics.push(elaboration_diagnostic(
                    "MNE201",
                    format!("{name} requires exactly {wanted} argument(s)"),
                    span,
                ));
                false
            }
        };
        let counter = BodyType::Integer(IntegerType {
            bits: 64,
            signed: false,
        });
        let lane_evidence = |expr: &AstExpr, lanes: u32| match expr {
            AstExpr::Integer { value, .. }
                if *value >= 0 && (*value as u128) < u128::from(lanes) =>
            {
                mncs_model::BoundsEvidence::StaticExact
            }
            _ => mncs_model::BoundsEvidence::RuntimeChecked {
                failure: FailureMode::Isolated,
            },
        };

        if name == "vector" {
            let Some(BodyType::Vector { element, lanes }) = expected else {
                diagnostics.push(elaboration_diagnostic(
                    "MNE202",
                    "vector construction requires an expected vec<T, N> type",
                    span,
                ));
                return None;
            };
            if arguments.len() != *lanes as usize {
                diagnostics.push(elaboration_diagnostic(
                    "MNE203",
                    format!("vector construction requires exactly {lanes} lane values"),
                    span,
                ));
                return None;
            }
            let mut operands = Vec::with_capacity(arguments.len());
            for argument in arguments {
                let binding = self.elaborate_expr(argument, Some(element), env, diagnostics)?;
                if binding.ty != **element {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE204",
                        "vector lane does not match the declared element type",
                        argument.span(),
                    ));
                    return None;
                }
                operands.push(binding.id);
            }
            return Some(
                self.emit_profile08_value(
                    "vec",
                    BodyOperationKind::VectorConstruct {
                        element_type: element.clone(),
                        lanes: *lanes,
                    },
                    operands,
                    expected
                        .cloned()
                        .unwrap_or_else(|| BodyType::Named("invalid".to_owned())),
                ),
            );
        }

        if name == "splat" {
            if !arity(1, diagnostics) {
                return None;
            }
            let Some(BodyType::Vector { element, lanes }) = expected else {
                diagnostics.push(elaboration_diagnostic(
                    "MNE205",
                    "vector splat requires an expected vec<T, N> type",
                    span,
                ));
                return None;
            };
            let value = self.elaborate_expr(&arguments[0], Some(element), env, diagnostics)?;
            return Some(
                self.emit_profile08_value(
                    "splat",
                    BodyOperationKind::VectorSplat {
                        element_type: element.clone(),
                        lanes: *lanes,
                    },
                    vec![value.id],
                    expected
                        .cloned()
                        .unwrap_or_else(|| BodyType::Named("invalid".to_owned())),
                ),
            );
        }

        if matches!(name, "extract_lane" | "replace_lane") {
            let wanted = if name == "extract_lane" { 2 } else { 3 };
            if !arity(wanted, diagnostics) {
                return None;
            }
            let vector = self.elaborate_expr(&arguments[0], None, env, diagnostics)?;
            let BodyType::Vector { element, lanes } = vector.ty.clone() else {
                diagnostics.push(elaboration_diagnostic(
                    "MNE206",
                    "lane operation requires a vector operand",
                    arguments[0].span(),
                ));
                return None;
            };
            if let AstExpr::Integer { value, .. } = &arguments[1] {
                if *value < 0 || *value >= i128::from(lanes) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE207",
                        format!("lane {value} is outside the vector domain 0..{lanes}"),
                        arguments[1].span(),
                    ));
                    return None;
                }
            }
            let index = self.elaborate_expr(&arguments[1], Some(&counter), env, diagnostics)?;
            let evidence = lane_evidence(&arguments[1], lanes);
            if name == "extract_lane" {
                return Some(self.emit_profile08_value(
                    "lane",
                    BodyOperationKind::VectorExtract {
                        element_type: element.clone(),
                        lanes,
                        evidence,
                    },
                    vec![vector.id, index.id],
                    *element,
                ));
            }
            let value = self.elaborate_expr(&arguments[2], Some(&element), env, diagnostics)?;
            return Some(self.emit_profile08_value(
                "vrep",
                BodyOperationKind::VectorReplace {
                    element_type: element.clone(),
                    lanes,
                    evidence,
                },
                vec![vector.id, index.id, value.id],
                BodyType::Vector { element, lanes },
            ));
        }

        if name.starts_with("vec_")
            && !matches!(
                name,
                "vec_eq" | "vec_ne" | "vec_lt" | "vec_le" | "vec_gt" | "vec_ge"
            )
        {
            if !arity(2, diagnostics) {
                return None;
            }
            let left = self.elaborate_expr(&arguments[0], expected, env, diagnostics)?;
            let BodyType::Vector { element, lanes } = left.ty.clone() else {
                diagnostics.push(elaboration_diagnostic(
                    "MNE208",
                    "vector binary operation requires a vector operand",
                    arguments[0].span(),
                ));
                return None;
            };
            if !matches!(element.as_ref(), BodyType::Integer(_)) {
                diagnostics.push(elaboration_diagnostic(
                    "MNE209",
                    "initial vector arithmetic supports integer lanes",
                    arguments[0].span(),
                ));
                return None;
            }
            let vector_type = BodyType::Vector {
                element: element.clone(),
                lanes,
            };
            let right = self.elaborate_expr(&arguments[1], Some(&vector_type), env, diagnostics)?;
            if right.ty != vector_type {
                diagnostics.push(elaboration_diagnostic(
                    "MNE210",
                    "vector operands must have identical type and lane count",
                    arguments[1].span(),
                ));
                return None;
            }
            let (operator, intent) = match name {
                "vec_add_wrap" => ("add", ArithmeticIntent::Wrapping),
                "vec_add_checked" => ("add", ArithmeticIntent::Checked),
                "vec_add_sat" => ("add", ArithmeticIntent::Saturating),
                "vec_sub_wrap" => ("sub", ArithmeticIntent::Wrapping),
                "vec_sub_checked" => ("sub", ArithmeticIntent::Checked),
                "vec_sub_sat" => ("sub", ArithmeticIntent::Saturating),
                "vec_mul_wrap" => ("mul", ArithmeticIntent::Wrapping),
                "vec_mul_checked" => ("mul", ArithmeticIntent::Checked),
                "vec_mul_sat" => ("mul", ArithmeticIntent::Saturating),
                "vec_and" => ("and", ArithmeticIntent::Wrapping),
                "vec_or" => ("or", ArithmeticIntent::Wrapping),
                "vec_xor" => ("xor", ArithmeticIntent::Wrapping),
                "vec_shl" => ("shl", ArithmeticIntent::Wrapping),
                "vec_shr" => ("shr", ArithmeticIntent::Wrapping),
                "vec_min" => ("min", ArithmeticIntent::Wrapping),
                "vec_max" => ("max", ArithmeticIntent::Wrapping),
                _ => {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE211",
                        "unknown vector operation",
                        span,
                    ));
                    return None;
                }
            };
            return Some(self.emit_profile08_value(
                "vop",
                BodyOperationKind::VectorBinary {
                    operator: operator.to_owned(),
                    element_type: element,
                    lanes,
                    intent,
                },
                vec![left.id, right.id],
                vector_type,
            ));
        }

        if matches!(
            name,
            "vec_eq" | "vec_ne" | "vec_lt" | "vec_le" | "vec_gt" | "vec_ge"
        ) {
            if !arity(2, diagnostics) {
                return None;
            }
            let left = self.elaborate_expr(&arguments[0], None, env, diagnostics)?;
            let BodyType::Vector { element, lanes } = left.ty.clone() else {
                diagnostics.push(elaboration_diagnostic(
                    "MNE212",
                    "vector comparison requires a vector operand",
                    arguments[0].span(),
                ));
                return None;
            };
            let vector_type = BodyType::Vector {
                element: element.clone(),
                lanes,
            };
            let right = self.elaborate_expr(&arguments[1], Some(&vector_type), env, diagnostics)?;
            if right.ty != vector_type {
                diagnostics.push(elaboration_diagnostic(
                    "MNE210",
                    "vector operands must have identical type and lane count",
                    arguments[1].span(),
                ));
                return None;
            }
            return Some(self.emit_profile08_value(
                "mask",
                BodyOperationKind::VectorCompare {
                    predicate: name.trim_start_matches("vec_").to_owned(),
                    element_type: element,
                    lanes,
                },
                vec![left.id, right.id],
                BodyType::Mask { lanes },
            ));
        }

        if matches!(name, "mask_and" | "mask_or" | "mask_xor") {
            if !arity(2, diagnostics) {
                return None;
            }
            let left = self.elaborate_expr(&arguments[0], None, env, diagnostics)?;
            let BodyType::Mask { lanes } = left.ty else {
                diagnostics.push(elaboration_diagnostic(
                    "MNE213",
                    "mask operation requires mask operands",
                    arguments[0].span(),
                ));
                return None;
            };
            let mask_type = BodyType::Mask { lanes };
            let right = self.elaborate_expr(&arguments[1], Some(&mask_type), env, diagnostics)?;
            if right.ty != mask_type {
                diagnostics.push(elaboration_diagnostic(
                    "MNE214",
                    "mask operands must have identical lane counts",
                    arguments[1].span(),
                ));
                return None;
            }
            return Some(self.emit_profile08_value(
                "mask",
                BodyOperationKind::MaskBinary {
                    operator: name.trim_start_matches("mask_").to_owned(),
                    lanes,
                },
                vec![left.id, right.id],
                mask_type,
            ));
        }

        if name == "mask_not" {
            if !arity(1, diagnostics) {
                return None;
            }
            let value = self.elaborate_expr(&arguments[0], None, env, diagnostics)?;
            let BodyType::Mask { lanes } = value.ty else {
                diagnostics.push(elaboration_diagnostic(
                    "MNE213",
                    "mask operation requires a mask operand",
                    arguments[0].span(),
                ));
                return None;
            };
            return Some(self.emit_profile08_value(
                "mask",
                BodyOperationKind::MaskNot { lanes },
                vec![value.id],
                BodyType::Mask { lanes },
            ));
        }

        if matches!(name, "mask_any" | "mask_all" | "mask_none") {
            if !arity(1, diagnostics) {
                return None;
            }
            let value = self.elaborate_expr(&arguments[0], None, env, diagnostics)?;
            let BodyType::Mask { lanes } = value.ty else {
                diagnostics.push(elaboration_diagnostic(
                    "MNE215",
                    "mask reduction requires a mask operand",
                    arguments[0].span(),
                ));
                return None;
            };
            return Some(self.emit_profile08_value(
                "mred",
                BodyOperationKind::MaskReduce {
                    operator: name.trim_start_matches("mask_").to_owned(),
                    lanes,
                },
                vec![value.id],
                BodyType::Named("bool".to_owned()),
            ));
        }

        if matches!(
            name,
            "reduce_sum_wrap" | "reduce_sum_checked" | "reduce_min" | "reduce_max"
        ) {
            if !arity(1, diagnostics) {
                return None;
            }
            let value = self.elaborate_expr(&arguments[0], None, env, diagnostics)?;
            let BodyType::Vector { element, lanes } = value.ty else {
                diagnostics.push(elaboration_diagnostic(
                    "MNE216",
                    "vector reduction requires a vector operand",
                    arguments[0].span(),
                ));
                return None;
            };
            let (operator, intent) = match name {
                "reduce_sum_wrap" => ("sum", ArithmeticIntent::Wrapping),
                "reduce_sum_checked" => ("sum", ArithmeticIntent::Checked),
                "reduce_min" => ("min", ArithmeticIntent::Wrapping),
                _ => ("max", ArithmeticIntent::Wrapping),
            };
            return Some(self.emit_profile08_value(
                "vred",
                BodyOperationKind::VectorReduce {
                    operator: operator.to_owned(),
                    element_type: element.clone(),
                    lanes,
                    intent,
                },
                vec![value.id],
                *element,
            ));
        }

        diagnostics.push(elaboration_diagnostic(
            "MNE211",
            format!("unknown Profile 0.8 intrinsic {name}"),
            span,
        ));
        None
    }

    fn emit_profile08_value(
        &mut self,
        prefix: &str,
        kind: BodyOperationKind,
        operands: Vec<String>,
        ty: BodyType,
    ) -> ResolvedBinding {
        let id = self.new_value(prefix);
        self.blocks[self.current].operations.push(BodyOperation {
            id: id.clone(),
            kind,
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
        ResolvedBinding::plain(id, ty)
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
        ResolvedBinding::plain(id, field_type)
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
    // Record fields and signatures are canonicalized to their declaring
    // nominal identity after linking. Keep the type resolver symmetric with
    // that representation so a later field projection can rehydrate the
    // same nominal type instead of treating the identity as an unknown name.
    if let Some(finite_type) = finite_types
        .values()
        .find(|candidate| candidate.identity.0 == name)
    {
        return BodyType::Finite {
            identity: finite_type.identity.clone(),
            name: finite_type.name.clone(),
        };
    }
    if let Some(record_type) = record_types
        .values()
        .find(|candidate| candidate.identity.0 == name)
    {
        return BodyType::Record {
            identity: record_type.identity.clone(),
            name: record_type.name.clone(),
        };
    }
    let parametric = BodyType::from_semantic_name(name);
    if matches!(parametric, BodyType::Mask { .. })
        || matches!(
            &parametric,
            BodyType::Vector { element, .. }
                if matches!(element.as_ref(), BodyType::Integer(IntegerType { bits: 8 | 16 | 32 | 64, .. }))
        )
    {
        return parametric;
    }
    if let Some(sequence) = profile_sequence_type(name, finite_types, record_types) {
        return sequence;
    }
    profile_scalar_supported(name).unwrap_or_else(|| {
        diagnostics.push(elaboration_diagnostic(
            "MNE105",
            "source profile supports bool, byte, 8/16/32/64-bit integers, declared finite/record types, bounded sequences, and Profile 0.8 integer vec<T, N>/mask<N>",
            span,
        ));
        BodyType::Named(name.to_owned())
    })
}

fn profile_type_with_generics(
    name: &str,
    span: SourceSpan,
    finite_types: &BTreeMap<String, FiniteType>,
    record_types: &BTreeMap<String, RecordType>,
    generics: &BTreeMap<String, mncs_model::GenericParamKind>,
    diagnostics: &mut Vec<SourceDiagnostic>,
) -> BodyType {
    // Direct type-parameter reference
    if let Some(kind) = generics.get(name) {
        return match kind {
            mncs_model::GenericParamKind::Type => BodyType::GenericParam {
                name: name.to_owned(),
            },
            mncs_model::GenericParamKind::Nat => {
                diagnostics.push(elaboration_diagnostic(
                    "MNE232",
                    format!("value parameter '{name}' cannot be used as a type"),
                    span,
                ));
                BodyType::Named(name.to_owned())
            }
        };
    }
    // Sequence types with possible generic element / bound
    if name.trim_start().starts_with('[') {
        if let Some(seq) = profile_sequence_type_with_generics(
            name,
            finite_types,
            record_types,
            generics,
            diagnostics,
            span,
        ) {
            return seq;
        }
    }
    // Defer to non-generic resolver (finite/record/scalar/sequence without generics)
    profile_type(name, span, finite_types, record_types, diagnostics)
}

fn substitute_body_type(
    ty: BodyType,
    type_map: &std::collections::BTreeMap<String, BodyType>,
    value_map: &std::collections::BTreeMap<String, u32>,
) -> BodyType {
    match ty {
        BodyType::GenericParam { name } => type_map
            .get(&name)
            .cloned()
            .unwrap_or(BodyType::GenericParam { name }),
        BodyType::Sequence { element, bound } => {
            let new_element = Box::new(substitute_body_type(*element, type_map, value_map));
            let new_bound = match bound {
                mncs_model::SequenceBound::Exact(v) => mncs_model::SequenceBound::Exact(v),
                mncs_model::SequenceBound::UpTo(v) => mncs_model::SequenceBound::UpTo(v),
                mncs_model::SequenceBound::Param(n) => {
                    if let Some(v) = value_map.get(&n) {
                        mncs_model::SequenceBound::Exact(*v)
                    } else {
                        mncs_model::SequenceBound::Param(n)
                    }
                }
                mncs_model::SequenceBound::UpToParam(n) => {
                    if let Some(v) = value_map.get(&n) {
                        mncs_model::SequenceBound::UpTo(*v)
                    } else {
                        mncs_model::SequenceBound::UpToParam(n)
                    }
                }
            };
            BodyType::Sequence {
                element: new_element,
                bound: new_bound,
            }
        }
        BodyType::Vector { element, lanes } => BodyType::Vector {
            element: Box::new(substitute_body_type(*element, type_map, value_map)),
            lanes,
        },
        BodyType::Mask { lanes } => BodyType::Mask { lanes },
        BodyType::Record { identity, name } => BodyType::Record { identity, name },
        BodyType::Finite { identity, name } => BodyType::Finite { identity, name },
        BodyType::Integer(i) => BodyType::Integer(i),
        BodyType::Byte => BodyType::Byte,
        BodyType::Named(n) => BodyType::Named(n),
    }
}

fn canonical_value_type(source: &str, ty: &BodyType) -> String {
    match ty {
        BodyType::Sequence { element, bound } => {
            let inner_source = source
                .strip_prefix('[')
                .and_then(|text| text.strip_suffix(']'))
                .and_then(|inner| {
                    let separator = inner.rfind(';')?;
                    Some(inner[..separator].trim().to_owned())
                })
                .unwrap_or_else(|| element.semantic_name());
            format!(
                "[{}; {}]",
                canonical_value_type(&inner_source, element),
                bound.canonical_text()
            )
        }
        BodyType::Finite { identity, .. } | BodyType::Record { identity, .. }
            if source.contains('.') =>
        {
            identity.0.clone()
        }
        _ => source.to_owned(),
    }
}

fn canonical_imported_sequence_type(source: &str, ty: &BodyType) -> String {
    let BodyType::Sequence { element, bound } = ty else {
        return canonical_value_type(source, ty);
    };
    let inner_source = source
        .strip_prefix('[')
        .and_then(|text| text.strip_suffix(']'))
        .and_then(|inner| {
            let separator = inner.rfind(';')?;
            Some(inner[..separator].trim().to_owned())
        })
        .unwrap_or_else(|| element.semantic_name());
    format!(
        "[{}; {}]",
        canonical_sequence_element_type(&inner_source, element),
        bound.canonical_text()
    )
}

fn canonical_sequence_element_type(source: &str, ty: &BodyType) -> String {
    match ty {
        BodyType::Finite { identity, .. } | BodyType::Record { identity, .. } => identity.0.clone(),
        BodyType::Sequence { element, bound } => {
            format!(
                "[{}; {}]",
                canonical_sequence_element_type(source, element),
                bound.canonical_text()
            )
        }
        _ => canonical_value_type(source, ty),
    }
}

/// Resolve a canonical bounded-sequence spelling `[E; N]` / `[E; up_to M]`
/// whose element may be a scalar, byte, nested sequence, or a declared
/// nominal type of this module.
fn profile_sequence_type(
    name: &str,
    finite_types: &BTreeMap<String, FiniteType>,
    record_types: &BTreeMap<String, RecordType>,
) -> Option<BodyType> {
    let inner = name.strip_prefix('[')?.strip_suffix(']')?;
    let separator = inner.rfind(';')?;
    let (element_text, bound_text) = (inner[..separator].trim(), inner[separator + 1..].trim());
    let bound = if let Some(capacity) = bound_text.strip_prefix("up_to") {
        let capacity = capacity.trim().parse::<u32>().ok()?;
        if capacity > mncs_model::MAX_SEQUENCE_BOUND {
            return None;
        }
        mncs_model::SequenceBound::UpTo(capacity)
    } else {
        let length = bound_text.parse::<u32>().ok()?;
        if length > mncs_model::MAX_SEQUENCE_BOUND {
            return None;
        }
        mncs_model::SequenceBound::Exact(length)
    };
    let finite = finite_types.get(element_text).or_else(|| {
        finite_types
            .values()
            .find(|candidate| candidate.identity.0 == element_text)
    });
    let record = record_types.get(element_text).or_else(|| {
        record_types
            .values()
            .find(|candidate| candidate.identity.0 == element_text)
    });
    if finite.is_some() || record.is_some() {
        // Nominal elements resolve through profile_type on a fresh diagnostic
        // budget; reuse the direct maps to keep this helper side-effect free.
        if let Some(finite) = finite {
            return Some(BodyType::Sequence {
                element: Box::new(BodyType::Finite {
                    identity: finite.identity.clone(),
                    name: finite.name.clone(),
                }),
                bound,
            });
        }
        let record = record?;
        return Some(BodyType::Sequence {
            element: Box::new(BodyType::Record {
                identity: record.identity.clone(),
                name: record.name.clone(),
            }),
            bound,
        });
    }
    let element = Box::new(match profile_scalar_supported(element_text) {
        Some(scalar) => scalar,
        None => profile_sequence_type(element_text, finite_types, record_types)?,
    });
    if matches!(&*element, BodyType::Mask { .. } | BodyType::Vector { .. }) {
        return None;
    }
    match &*element {
        BodyType::Named(name) if name != "bool" => None,
        _ => Some(BodyType::Sequence { element, bound }),
    }
}

fn profile_sequence_type_with_generics(
    name: &str,
    finite_types: &BTreeMap<String, FiniteType>,
    record_types: &BTreeMap<String, RecordType>,
    generics: &BTreeMap<String, mncs_model::GenericParamKind>,
    diagnostics: &mut Vec<SourceDiagnostic>,
    span: SourceSpan,
) -> Option<BodyType> {
    let inner = name.strip_prefix('[')?.strip_suffix(']')?;
    let separator = inner.rfind(';')?;
    let (element_text, bound_text) = (inner[..separator].trim(), inner[separator + 1..].trim());
    let bound = if let Some(capacity) = bound_text.strip_prefix("up_to") {
        let cap = capacity.trim();
        if let Ok(v) = cap.parse::<u32>() {
            if v > mncs_model::MAX_SEQUENCE_BOUND {
                return None;
            }
            mncs_model::SequenceBound::UpTo(v)
        } else if generics
            .get(cap)
            .is_some_and(|k| *k == mncs_model::GenericParamKind::Nat)
        {
            mncs_model::SequenceBound::UpToParam(cap.to_owned())
        } else if cap.chars().all(|c| c.is_alphanumeric() || c == '_') {
            diagnostics.push(elaboration_diagnostic(
                "MNE224",
                format!("unknown generic value parameter '{cap}' in view bound"),
                span,
            ));
            return None;
        } else {
            return None;
        }
    } else if let Ok(v) = bound_text.parse::<u32>() {
        if v > mncs_model::MAX_SEQUENCE_BOUND {
            return None;
        }
        mncs_model::SequenceBound::Exact(v)
    } else if generics
        .get(bound_text)
        .is_some_and(|k| *k == mncs_model::GenericParamKind::Nat)
    {
        mncs_model::SequenceBound::Param(bound_text.to_owned())
    } else if bound_text.chars().all(|c| c.is_alphanumeric() || c == '_') {
        diagnostics.push(elaboration_diagnostic(
            "MNE224",
            format!("unknown generic value parameter '{bound_text}' in sequence bound"),
            span,
        ));
        return None;
    } else {
        return None;
    };
    // Resolve element
    let element = if let Some(kind) = generics.get(element_text) {
        if *kind == mncs_model::GenericParamKind::Type {
            BodyType::GenericParam {
                name: element_text.to_owned(),
            }
        } else {
            diagnostics.push(elaboration_diagnostic(
                "MNE232",
                format!("value parameter '{element_text}' cannot be used as sequence element type"),
                span,
            ));
            return None;
        }
    } else if let Some(finite) = finite_types.get(element_text).or_else(|| {
        finite_types
            .values()
            .find(|candidate| candidate.identity.0 == element_text)
    }) {
        BodyType::Finite {
            identity: finite.identity.clone(),
            name: finite.name.clone(),
        }
    } else if let Some(record) = record_types.get(element_text).or_else(|| {
        record_types
            .values()
            .find(|candidate| candidate.identity.0 == element_text)
    }) {
        BodyType::Record {
            identity: record.identity.clone(),
            name: record.name.clone(),
        }
    } else if let Some(scalar) = profile_scalar_supported(element_text) {
        scalar
    } else if let Some(nested) = profile_sequence_type_with_generics(
        element_text,
        finite_types,
        record_types,
        generics,
        diagnostics,
        span,
    ) {
        nested
    } else {
        // Try scalar via from_semantic_name for bool etc.
        let ty = BodyType::from_semantic_name(element_text);
        match &ty {
            BodyType::Named(n) if n != "bool" => return None,
            _ => ty,
        }
    };
    if matches!(&element, BodyType::Mask { .. } | BodyType::Vector { .. }) {
        return None;
    }
    Some(BodyType::Sequence {
        element: Box::new(element),
        bound,
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
            }) | BodyType::Byte
        );
    supported.then_some(ty)
}

fn profile_type_supported_for_generic_kind(
    name: &str,
    finite_types: &BTreeMap<String, FiniteType>,
    record_types: &BTreeMap<String, RecordType>,
) -> bool {
    profile_scalar_supported(name).is_some()
        || finite_types.contains_key(name)
        || record_types.contains_key(name)
        || name.contains('.')
        || name.starts_with('[')
        || name.starts_with("vec<")
        || name.starts_with("mask<")
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
