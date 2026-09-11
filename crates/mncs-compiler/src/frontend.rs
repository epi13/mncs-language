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
    ContractKind, Effect, FailureMode, FiniteType, FiniteVariant, FloatType, Function,
    FunctionBody, IntegerType, Intent, IterationDomain, MachineIntentSpec, MachinePreference,
    Program, RecordField, RecordType, Requirement, ResolutionProvenance, SemanticBinding,
    SemanticBindingKind, SemanticBindingTable, SemanticGraph, SemanticId, SemanticIdentities,
    SemanticNamespace, SemanticReference, SemanticScope, TransformationEdge, TransformationStatus,
    ValidationReport, Value, EXECUTABLE_BODY_SCHEMA_VERSION, SUPPORTED_SCHEMA_VERSION,
};
use mncs_syntax::{
    parse, AbstractSyntaxTree, AstBinaryOp, AstExpr, AstFunction, AstMatchArm, AstMatchPattern,
    AstStmt, ConcreteSyntaxTree, DiagnosticSeverity, DiagnosticStage, LexedDocument, ParseOutput,
    SourceArtifactKind, SourceDiagnostic, SourceEnvelope, SourceSpan, SpannedText,
    AST_SCHEMA_VERSION, CST_SCHEMA_VERSION, LEXICAL_SCHEMA_VERSION, SOURCE_ENVELOPE_SCHEMA_VERSION,
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

    /// Detailed resolution distinguishing a miss from conflicting
    /// candidates (HARNESS-PRESSURE-008). The default derives from
    /// `resolve`, reporting no conflict information; resolvers that search
    /// several roots override it so duplicate identities fail closed.
    fn resolve_detailed(&self, module: &str) -> ModuleResolutionOutcome {
        match self.resolve(module) {
            Some(envelope) => ModuleResolutionOutcome::Resolved(Box::new(envelope)),
            None => ModuleResolutionOutcome::NotFound,
        }
    }
}

/// The detailed outcome of one module-resolution query.
#[derive(Debug, Clone)]
pub enum ModuleResolutionOutcome {
    /// Exactly one authoritative candidate. Boxed: resolution is a cold
    /// elaboration-time path and the envelope dwarfs the other variants.
    Resolved(Box<SourceEnvelope>),
    /// No candidate satisfied the requested name.
    NotFound,
    /// Several distinct candidates satisfy the name. The strings identify
    /// the conflicting sources (paths or locators); resolution fails closed
    /// rather than silently preferring one authority.
    Conflict(Vec<String>),
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
    let mut module_ceilings = BTreeMap::new();
    let result = if let Err(mut errors) = elaborate_import_closure(
        ast,
        &recording_resolver,
        &mut elaborated,
        &mut declaration_spans,
        &mut visiting,
        &mut module_ceilings,
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
                Ok(specialized) => {
                    // Admitted-ceiling enforcement for concrete bounds
                    // substituted into generic specializations (RFC 0036).
                    // Generic definitions defer the admitted check, so each
                    // specialization's concrete traversal bounds are checked
                    // here against the admitted ceiling of the module that
                    // defines the traversal: explicit Nat arguments already
                    // pass MNE225 at their call site, and this sweep closes
                    // the remaining paths (inference, cross-module
                    // substitution). A narrow root never un-admits library
                    // internals admitted under the library's own profile.
                    // Non-generic functions keep exactly their
                    // definition-site behavior.
                    let mut ceiling_errors = specialized_traversal_ceiling_errors(
                        &specialized,
                        &module_ceilings,
                        mncs_syntax::max_sequence_bound_for(&ast.language_version.text)
                            .unwrap_or(0),
                        ast.module.span,
                    );
                    if ceiling_errors.is_empty() {
                        Ok(specialized)
                    } else {
                        diagnostics.append(&mut ceiling_errors);
                        Err(diagnostics)
                    }
                }
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

    fn resolve_detailed(&self, module: &str) -> ModuleResolutionOutcome {
        let outcome = self.inner.resolve_detailed(module);
        if let ModuleResolutionOutcome::Resolved(source) = &outcome {
            self.sources
                .borrow_mut()
                .entry(module.to_owned())
                .or_insert_with(|| source.as_ref().clone());
        }
        outcome
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
    module_ceilings: &mut BTreeMap<String, u32>,
) -> Result<(), Vec<SourceDiagnostic>> {
    let name = ast.module.text.clone();
    if elaborated.contains_key(&name) {
        return Ok(());
    }
    // RFC 0036: each module's admitted sequence ceiling travels with the
    // closure so the post-specialization sweep can check every substituted
    // traversal bound against the ceiling of the module that defines the
    // traversal — not the root's. A narrow root never un-admits library
    // internals admitted under the library's own profile, and a wide
    // caller never smuggles an over-ceiling instantiation past the
    // defining module's ceiling.
    module_ceilings.insert(
        name.clone(),
        mncs_syntax::max_sequence_bound_for(&ast.language_version.text).unwrap_or(0),
    );
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
        let dependency_envelope = match resolver.resolve_detailed(&dependency_name) {
            ModuleResolutionOutcome::Resolved(envelope) => *envelope,
            ModuleResolutionOutcome::NotFound => {
                return Err(vec![elaboration_diagnostic(
                    "MNE173",
                    format!("imported module '{dependency_name}' is unavailable to the resolver"),
                    use_decl.module.span,
                )]);
            }
            ModuleResolutionOutcome::Conflict(sources) => {
                return Err(vec![elaboration_diagnostic(
                    "MNE234",
                    format!(
                        "imported module '{dependency_name}' resolves to conflicting candidates [{}]; duplicate module identities are rejected",
                        sources.join(", ")
                    ),
                    use_decl.module.span,
                )]);
            }
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
            module_ceilings,
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
        // Import-path re-resolution of an already-validated artifact:
        // absolute model ceiling. Admission was enforced at origin
        // elaboration under the origin profile.
        mncs_model::MODEL_MAX_SEQUENCE_BOUND,
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
    // Payload type universe: variant payloads resolve against the same
    // namespace record fields do — local declarations (seeded with
    // computable nominal identities, exactly as the record-field pass
    // seeds its provisional map), imported bindings, and alias-qualified
    // names. Payloads therefore carry ordinary valid value types
    // (scalars, finite, records including cross-module, bounded sequences
    // including nested/nominal) instead of raw source spellings
    // (ENG-PRESSURE-0007).
    let mut payload_finite_types: BTreeMap<String, FiniteType> = ast
        .finite_types
        .iter()
        .map(|declaration| {
            (
                declaration.name.text.clone(),
                FiniteType {
                    identity: finite_type_id(&ast.module.text, &declaration.name.text),
                    name: declaration.name.text.clone(),
                    variants: Vec::new(),
                },
            )
        })
        .collect();
    payload_finite_types.extend(context.finite_types_by_name.clone());
    payload_finite_types.extend(context.qualified_finite_types.clone());
    let mut payload_record_types: BTreeMap<String, RecordType> = ast
        .record_types
        .iter()
        .map(|declaration| {
            (
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
            )
        })
        .collect();
    payload_record_types.extend(context.record_types_by_name.clone());
    payload_record_types.extend(context.qualified_record_types.clone());
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
            // and the record-field type universe — scalars, finite, records
            // (including cross-module), and bounded sequences (including
            // nested/nominal) — stored under canonical spellings so body
            // validation, lowering, and the ABI rehydrate the same nominal
            // types (ENG-PRESSURE-0007).
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
                let mut probe_diagnostics = Vec::new();
                let payload_ty = profile_type(
                    &field.value_type.text,
                    field.value_type.span,
                    &payload_finite_types,
                    &payload_record_types,
                    &mut probe_diagnostics,
                    admitted_sequence_ceiling(ast),
                );
                let supported = probe_diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.code != "MNE105")
                    && !matches!(payload_ty, BodyType::Named(_));
                if !supported {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE171",
                        "variant payload field type does not name a supported scalar, finite, record, or bounded-sequence type",
                        field.value_type.span,
                    ));
                    continue;
                }
                payload_fields.push(RecordField {
                    name: field.name.text.clone(),
                    field_type: canonical_value_type(&field.value_type.text, &payload_ty),
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
                    admitted_sequence_ceiling(ast),
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
                admitted_sequence_ceiling(ast),
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
            projected_from: None,
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
            projected_from: None,
        });
        for variant in &finite_type.variants {
            bindings.push(SemanticBinding {
                identity: binding_id_for(&module_scope, "finite-variant", &variant.identity),
                spelling: variant.name.clone(),
                namespace: namespace.clone(),
                scope: module_scope.clone(),
                declaration: variant.identity.clone(),
                kind: SemanticBindingKind::FiniteVariant,
                projected_from: None,
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
            projected_from: None,
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
                projected_from: None,
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
            projected_from: resolution.projected_from.clone(),
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
                admitted_sequence_ceiling(ast),
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
        admitted_sequence_ceiling(ast),
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
                "property" => ContractKind::Property,
                "invariant" => ContractKind::Invariant,
                "metamorphic" => ContractKind::Metamorphic,
                _ => {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE112",
                        "unsupported contract clause",
                        clause.span,
                    ));
                    return None;
                }
            };
            // Executable contract bindings name an MNCS predicate function in
            // the same program. The binding is checked here so a contract is
            // machine-consumable from elaboration onward: tooling can execute
            // the predicate directly instead of trusting an opaque string.
            // Legacy `requires`/`ensures`/`assumes` names stay unchecked.
            if matches!(
                kind,
                ContractKind::Property | ContractKind::Invariant | ContractKind::Metamorphic
            ) {
                if !mncs_syntax::profile_at_least(
                    &ast.language_version.text,
                    SOURCE_PROFILE_VERSION_0_9,
                ) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE230",
                        "executable contract clauses require source profile 0.9 or later",
                        clause.span,
                    ));
                    return None;
                }
                match signatures.get(&clause.name.text) {
                    None => {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE231",
                            "executable contract clause names an unknown function",
                            clause.span,
                        ));
                        return None;
                    }
                    Some(signature) => {
                        if !matches!(&signature.output, BodyType::Named(name) if name == "bool") {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE233",
                                "executable contract predicate must return bool",
                                clause.span,
                            ));
                            return None;
                        }
                    }
                }
            }
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
    // RFC 0047 designated measure: the first parameter's binding (with its
    // body value, which is the parameter name) seeds descendant provenance.
    // A refused binding designates nothing, failing self-calls closed.
    let mut measure_binding: Option<SemanticId> = None;
    for (index, (input, parameter)) in function.inputs.iter().zip(&parameters).enumerate() {
        let bound = env.bind(
            input.name.text.clone(),
            input.name.text.clone(),
            parameter.ty.clone(),
            input.name.span,
            BoundNameKind::Parameter,
            &mut diagnostics,
        );
        if index == 0 {
            measure_binding = bound;
        }
    }
    let measure = measure_binding.map(|binding| {
        (
            binding,
            function
                .inputs
                .first()
                .map(|input| input.name.text.clone())
                .unwrap_or_default(),
        )
    });
    // A bodyless tail `return name;` whose type differs from the declared output
    // only by the exact-to-bounded-view borrow takes the general path, which
    // elaborates the returned expression against the output type and
    // synthesizes the borrow there. The probe resolves against a cloned
    // environment so no resolution or diagnostic leaks from the check itself.
    let tail_name = match &function.body.returned_value {
        AstExpr::Name(returned_name) => Some(returned_name),
        _ => None,
    };
    let tail_borrows = tail_name.is_some_and(|returned_name| {
        let mut probe = env.clone();
        let mut sink = Vec::new();
        probe
            .resolve(&returned_name.text, returned_name.span, &mut sink)
            .is_some_and(|returned| {
                returned.ty != output_type
                    && exact_view_borrow_dimensions(&returned.ty, &output_type).is_some()
            })
    });
    let (blocks, bounded_iterations, builder_resolutions, structural_evidence) =
        if function.body.statements.is_empty() && tail_name.is_some() && !tail_borrows {
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
                ast.language_version.text.clone(),
                measure.clone(),
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
            let structural_evidence = std::mem::take(&mut builder.structural_evidence);
            (
                builder.blocks,
                builder.bounded_iterations,
                builder_resolutions,
                structural_evidence,
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
        evidence: structural_evidence
            .into_iter()
            .map(|claim| mncs_model::EvidenceClaim {
                property: mncs_model::STRUCTURAL_DECREASE_PROPERTY.to_owned(),
                verifier: "structural-decrease-rederivation".to_owned(),
                status: mncs_model::EvidenceStatus::Claimed,
                artifact: serde_json::to_string(&claim).ok(),
            })
            .collect(),
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
                    admitted_sequence_ceiling(ast),
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
                    admitted_sequence_ceiling(ast),
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

/// Admitted-ceiling enforcement for concrete traversal bounds substituted
/// into generic specializations (RFC 0036). Generic definitions defer the
/// admitted check (see the iterate-domain lowering), so this sweep checks
/// every specialization's concrete `Exact`/`UpTo` traversal bounds against
/// the admitted ceiling of the module that defines the traversal.
/// `module_ceilings` carries each closure member's ceiling; `fallback` (the
/// root program's ceiling) covers defining modules the closure did not
/// record. Symbolic bounds that forward to an outer caller stay deferred
/// until that caller specializes. Only functions named by specialization
/// records are examined, so non-generic programs keep exactly their
/// definition-site behavior.
fn specialized_traversal_ceiling_errors(
    program: &Program,
    module_ceilings: &BTreeMap<String, u32>,
    fallback: u32,
    span: SourceSpan,
) -> Vec<SourceDiagnostic> {
    // One shared implementation with the model so the
    // definition/instantiation contract cannot drift between layers.
    // Keyed like `specialization_function` (see generics.rs).
    let by_id: BTreeMap<SemanticId, &Function> = program
        .functions
        .iter()
        .map(|function| (function_identity_of(program, function), function))
        .collect();
    let mut errors = Vec::new();
    for record in &program.generic_specializations {
        let Some(function) = by_id.get(&record.specialization_function) else {
            continue;
        };
        let Some(body) = function.body.as_ref() else {
            continue;
        };
        // The traversal text lives in the defining module, so its profile
        // governs the substituted bound: a narrow root never un-admits
        // library internals, and a wide caller never smuggles an
        // over-ceiling instantiation past the definition site.
        let home = function.identity_namespace(&program.module);
        let admitted = module_ceilings.get(home).copied().unwrap_or(fallback);
        if !mncs_model::specialized_bounds_over_ceiling(body, admitted).is_empty() {
            errors.push(elaboration_diagnostic(
                "MNE182",
                format!("specialized traversal bound exceeds the profile ceiling {admitted}"),
                span,
            ));
        }
    }
    errors
}

/// Semantic identity of one linked function, matching the keys that
/// generic specialization records use.
fn function_identity_of(program: &Program, function: &Function) -> SemanticId {
    function_id(function.identity_namespace(&program.module), &function.name)
}

fn reject_recursive_calls(
    ast: &AbstractSyntaxTree,
    signatures: &BTreeMap<String, FunctionSignature>,
    diagnostics: &mut Vec<SourceDiagnostic>,
) {
    // RFC 0047 (Source Profile 0.13+): direct self-calls leave the
    // syntactic graph here and reach the elaboration-time structural check,
    // which decides over resolved bindings. Mutual and indirect cycles
    // stay syntactic rejections on every profile, and older profiles keep
    // the historical behavior of rejecting every self-cycle up front.
    let structural_recursion = mncs_syntax::profile_at_least(
        &ast.language_version.text,
        mncs_syntax::SOURCE_PROFILE_VERSION_0_13,
    );
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
            if structural_recursion {
                calls.remove(&function.name.text);
            }
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
        AstExpr::CheckedIndex {
            sequence, index, ..
        } => {
            calls_in_expr(sequence, calls);
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
        AstExpr::Not { value, .. } => calls_in_expr(value, calls),
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
        AstExpr::SequenceCopy {
            destination,
            dst_at,
            source,
            src_at,
            len,
            ..
        } => {
            calls_in_expr(destination, calls);
            calls_in_expr(dst_at, calls);
            calls_in_expr(source, calls);
            calls_in_expr(src_at, calls);
            calls_in_expr(len, calls);
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
        AstExpr::SequenceRepeat { element, .. } => {
            calls_in_expr(element, calls);
        }
        AstExpr::Sha256Digest { view, .. } => calls_in_expr(view, calls),
        AstExpr::HostWrite { view, .. } => calls_in_expr(view, calls),
        AstExpr::FloatIntrinsic { argument, .. } => calls_in_expr(argument, calls),
        AstExpr::Ed25519Verify {
            pubkey,
            message,
            signature,
            ..
        } => {
            calls_in_expr(pubkey, calls);
            calls_in_expr(message, calls);
            calls_in_expr(signature, calls);
        }
        AstExpr::FsEntryNameAt { index, .. } | AstExpr::FsEntryKindAt { index, .. } => {
            calls_in_expr(index, calls)
        }
        AstExpr::FsReadBytesAt {
            entry,
            offset,
            length,
            ..
        } => {
            calls_in_expr(entry, calls);
            calls_in_expr(offset, calls);
            calls_in_expr(length, calls);
        }
        AstExpr::Name(_)
        | AstExpr::QualifiedPath { .. }
        | AstExpr::Integer { .. }
        | AstExpr::Float { .. }
        | AstExpr::Boolean { .. }
        | AstExpr::HostRead { .. }
        | AstExpr::ClockRead { .. }
        | AstExpr::FsListCount { .. }
        | AstExpr::FsGeneration { .. }
        | AstExpr::FiniteVariant { .. } => {}
    }
}

#[derive(Clone)]
struct BindingEnv {
    scopes: Vec<std::collections::BTreeMap<String, (ResolvedBinding, SourceSpan, BoundNameKind)>>,
    scope_ids: Vec<SemanticId>,
    scope_paths: Vec<String>,
    /// Monotonic slot allocator per scope. Slots feed `binding_id`, so two
    /// declarations must never share one — even when a later declaration
    /// shadows an earlier name in the same scope (ENG-PRESSURE-0021) and the
    /// map length no longer advances.
    scope_slots: Vec<usize>,
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

/// Structural-recursion descendant provenance (RFC 0047) for one resolved
/// binding: either the designated measure itself or a match-payload
/// projection from a parent binding through one projection operation.
/// Bindings with no entry here (`let` aliases, call results, rebuilt
/// values, arithmetic, iteration state) are opaque and never count as
/// structural descendants, even when the spelling coincides.
#[derive(Debug, Clone)]
enum RecursionProvenance {
    Measure,
    Projected {
        parent_binding: SemanticId,
        parent_value: String,
        op: String,
    },
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
    /// The index binding of an enclosing counted loop (Profile 0.11). It
    /// carries the 0-based position but never discharges element
    /// projections: only traversal semantics prove positions in bounds.
    CountedIndex,
}

impl BoundNameKind {
    fn resolved_kind(self) -> ResolvedNameKind {
        match self {
            Self::Parameter => ResolvedNameKind::Parameter,
            Self::Binding => ResolvedNameKind::Binding,
            Self::IterationState | Self::TraversalIndex | Self::CountedIndex => {
                ResolvedNameKind::IterationState
            }
        }
    }

    fn binding_kind(self) -> &'static str {
        match self {
            Self::Parameter => "parameter",
            Self::Binding => "local",
            Self::IterationState => "iteration-state",
            Self::TraversalIndex | Self::CountedIndex => "iteration-index",
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
            scope_slots: vec![0],
            next_scope: 0,
            namespace,
            owner,
            resolutions: Vec::new(),
        }
    }

    /// Bind one name, returning the fresh binding identity (or `None` when
    /// the binding was refused with `MNE110`). Callers that do not track
    /// descendant provenance ignore the return.
    fn bind(
        &mut self,
        name: String,
        id: String,
        ty: BodyType,
        declaration: SourceSpan,
        kind: BoundNameKind,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<SemanticId> {
        self.bind_inner(name, id, ty, declaration, kind, None, diagnostics)
    }

    /// Bind one match-payload name projected from `subject` (RFC 0047): the
    /// declaration resolution records the subject binding so the kernel can
    /// re-derive the descendant chain. A subject without a resolved binding
    /// (call results, rebuilt values) binds exactly like an opaque `let`.
    fn bind_projection(
        &mut self,
        name: String,
        id: String,
        ty: BodyType,
        declaration: SourceSpan,
        subject: Option<SemanticId>,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<SemanticId> {
        self.bind_inner(
            name,
            id,
            ty,
            declaration,
            BoundNameKind::Binding,
            subject,
            diagnostics,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn bind_inner(
        &mut self,
        name: String,
        id: String,
        ty: BodyType,
        declaration: SourceSpan,
        kind: BoundNameKind,
        projection_subject: Option<SemanticId>,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<SemanticId> {
        let scope_index = self.scopes.len() - 1;
        // ENG-PRESSURE-0021: a plain `let` may rebind a name already bound
        // to a plain value (a `let` or a parameter) in the same scope. The
        // initializer already elaborated against the previous binding, and
        // the fresh SSA value keeps every use dominated by its own
        // definition, so rebinding is shadowing, not mutation. Index and
        // iteration-state names stay reserved: rebinding over (or under)
        // them keeps `MNE110`, so traversal-discharge reasoning, which keys
        // off the scope map, cannot silently change meaning.
        if let Some((_, _, existing_kind)) = self
            .scopes
            .get(scope_index)
            .expect("scope stack is non-empty")
            .get(&name)
        {
            let shadows_value = matches!(
                existing_kind,
                BoundNameKind::Binding | BoundNameKind::Parameter
            ) && matches!(kind, BoundNameKind::Binding);
            if !shadows_value {
                diagnostics.push(elaboration_diagnostic(
                    "MNE110",
                    "binding is ambiguous in this lexical scope",
                    declaration,
                ));
                return None;
            }
        }
        let slot = self.scope_slots[scope_index];
        self.scope_slots[scope_index] += 1;
        let scope = self
            .scopes
            .get_mut(scope_index)
            .expect("scope stack is non-empty");
        let scope_identity = self.scope_ids[scope_index].clone();
        let binding = binding_id(&scope_identity, kind.binding_kind(), slot);
        let mut resolution = NameResolution::new(declaration, declaration, kind.resolved_kind())
            .with_binding_metadata(
                binding.clone(),
                self.namespace.clone(),
                scope_identity.clone(),
                vec![name.clone()],
                ResolutionProvenance::Local,
                binding.clone(),
            );
        if let Some(subject) = projection_subject {
            resolution = resolution.with_projection_subject(subject);
        }
        self.resolutions.push(resolution);
        scope.insert(
            name.clone(),
            (
                ResolvedBinding {
                    id,
                    ty,
                    binding: Some(binding.clone()),
                    namespace: Some(self.namespace.clone()),
                    scope: Some(scope_identity),
                    path: vec![name],
                    provenance: Some(ResolutionProvenance::Local),
                },
                declaration,
                kind,
            ),
        );
        Some(binding)
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

    /// Whether `name` currently resolves to a bounded-traversal index.
    /// The innermost binding wins, exactly as name resolution does: a
    /// nested `let` shadowing the index name resolves uses to the plain
    /// value, so those uses must not inherit the traversal-domain
    /// discharge (ENG-PRESSURE-0021 companion fix; previously `.any()`
    /// discharged through the shadow).
    fn is_traversal_index(&self, name: &str) -> bool {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .is_some_and(|(_, _, kind)| matches!(kind, BoundNameKind::TraversalIndex))
    }

    fn binds(&self, name: &str) -> bool {
        self.scopes
            .iter()
            .rev()
            .any(|scope| scope.contains_key(name))
    }

    /// Whether a plain `let` for `name` would shadow a plain value (a `let`
    /// or a parameter) in the current scope. Mirrors the `shadows_value`
    /// condition in [`Self::bind`]: same-scope shadowing is a Profile 0.13
    /// extension (ENG-PRESSURE-0021); older profiles keep the historical
    /// MNE110 refusal, enforced by the caller before [`Self::bind`] runs.
    fn shadows_plain_value_in_current_scope(&self, name: &str) -> bool {
        self.scopes
            .last()
            .expect("scope stack is non-empty")
            .get(name)
            .is_some_and(|(_, _, existing_kind)| {
                matches!(
                    existing_kind,
                    BoundNameKind::Binding | BoundNameKind::Parameter
                )
            })
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
        self.scope_slots.push(0);
    }

    fn pop(&mut self) {
        self.scopes.pop();
        self.scope_ids.pop();
        self.scope_paths.pop();
        self.scope_slots.pop();
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
    iteration_depth: usize,
    /// Lexically enclosing `iterate` loops as `(source name, static bound)`
    /// (Profile 0.13, CP-0009). Iteration identities are unique over their
    /// live lexical scope: a nested loop may not reuse a still-open
    /// identity, while sequential non-overlapping loops may. The bound
    /// feeds the compositional static work-product rule. Pushed at loop
    /// entry, popped once the body (the only place a nested loop can
    /// open) is elaborated, including on error exits below.
    open_iterations: Vec<(String, u32)>,
    /// Per-name elaboration counts for hygienic recorded identities
    /// (CP-0009). The first loop with a source name keeps it verbatim;
    /// later sequential reuses record `name#2`, `name#3`, ... so the
    /// proof graph, obligation subjects, and MNB061 uniqueness stay
    /// per-loop even when the source name repeats.
    iteration_name_uses: BTreeMap<String, u64>,
    /// Checked-index facts (Profile 0.14, WEB-P-009): checked value id to
    /// the sequence value id the dominating `BoundCheck` verified it
    /// against. Value ids are globally fresh and branch/loop scopes pop
    /// names, so a hit means the check dominates the use; any merge or
    /// rebinding mints a fresh id and misses back to a runtime check.
    checked_indices: BTreeMap<String, String>,
    /// Declared source profile (`ast.language_version`), for additive
    /// feature gates. Older profiles keep their historical refusals and
    /// fingerprints; gates query through `profile_at_least`.
    source_profile: String,
    /// RFC 0047 descendant provenance over resolved bindings, seeded with
    /// the recursive parameter (`Measure`) when one is designated.
    recursion_provenance: BTreeMap<SemanticId, RecursionProvenance>,
    /// The designated structural measure: first-parameter binding and its
    /// body value, when the function declares at least one parameter.
    measure: Option<(SemanticId, String)>,
    /// Admitted structural-decrease records for this function (R5), drained
    /// into `Function.evidence` when elaboration succeeds.
    structural_evidence: Vec<mncs_model::StructuralDecreaseClaim>,
}

/// Admissible literal range of an integer type as `(min, max)` (CP-0010).
/// Defensive over the full `u16` width space: degenerate zero-width types
/// admit nothing, and widths at or past 128 saturate at the i128 extremes
/// rather than shifting out of range.
fn integer_range(operand_type: IntegerType) -> (i128, i128) {
    if operand_type.signed {
        if operand_type.bits >= 128 {
            (i128::MIN, i128::MAX)
        } else if operand_type.bits == 0 {
            (0, -1)
        } else {
            let half = 1i128 << (operand_type.bits - 1);
            (-half, half - 1)
        }
    } else if operand_type.bits >= 128 {
        (0, i128::MAX)
    } else if operand_type.bits == 0 {
        (0, -1)
    } else {
        (0, (1i128 << operand_type.bits) - 1)
    }
}

/// Parse a scalar match pattern literal: decimal digits with an optional
/// leading `-` tracked separately by the parser (CP-0010). Magnitudes past
/// i128 fail here with MNE145; subject-range checking happens at the call
/// site, so one diagnostic code covers the whole literal contract with no
/// silent truncation.
fn scalar_pattern_value(
    negative: bool,
    text: &SpannedText,
    diagnostics: &mut Vec<SourceDiagnostic>,
) -> Option<i128> {
    let magnitude: i128 = match text.text.parse() {
        Ok(value) => value,
        Err(_) => {
            diagnostics.push(elaboration_diagnostic(
                "MNE145",
                format!(
                    "scalar match literal {} is outside the subject integer type",
                    text.text
                ),
                text.span,
            ));
            return None;
        }
    };
    if !negative {
        return Some(magnitude);
    }
    match magnitude.checked_neg() {
        Some(value) => Some(value),
        None => {
            diagnostics.push(elaboration_diagnostic(
                "MNE145",
                format!(
                    "scalar match literal -{} is outside the subject integer type",
                    text.text
                ),
                text.span,
            ));
            None
        }
    }
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
        source_profile: String,
        measure: Option<(SemanticId, String)>,
    ) -> Self {
        let owner = function_id(&namespace, &function);
        let mut recursion_provenance = BTreeMap::new();
        if let Some((binding, _)) = &measure {
            recursion_provenance.insert(binding.clone(), RecursionProvenance::Measure);
        }
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
            open_iterations: Vec::new(),
            iteration_name_uses: BTreeMap::new(),
            checked_indices: BTreeMap::new(),
            iteration_depth: 0,
            source_profile,
            recursion_provenance,
            measure,
            structural_evidence: Vec::new(),
        }
    }

    fn profile_at_least(&self, version: &str) -> bool {
        mncs_syntax::profile_at_least(&self.source_profile, version)
    }

    fn profile_nested_iteration(&self) -> bool {
        self.profile_at_least(mncs_syntax::SOURCE_PROFILE_VERSION_0_11)
    }

    fn profile_float(&self) -> bool {
        self.profile_at_least(mncs_syntax::SOURCE_PROFILE_VERSION_0_12)
    }

    /// Profile 0.13 consolidation/progression capabilities (RFC 0036):
    /// boolean negation/equality, scalar integer match, sequential
    /// iteration-name reuse, contextual `next` fields, repeat literals,
    /// lexical shadowing, raised sequence/iteration ceilings, and
    /// structural recursion. Older profiles keep their historical
    /// acceptance/rejection behavior.
    fn profile_0_13(&self) -> bool {
        self.profile_at_least(mncs_syntax::SOURCE_PROFILE_VERSION_0_13)
    }

    /// Source Profile 0.14 (buffer pipelines): bulk span copy, checked
    /// view narrowing, and the checked-index discharge form.
    fn profile_0_14(&self) -> bool {
        self.profile_at_least(mncs_syntax::SOURCE_PROFILE_VERSION_0_14)
    }

    /// Admitted sequence/view length ceiling for the active profile.
    fn admitted_sequence_ceiling(&self) -> u32 {
        mncs_syntax::max_sequence_bound_for(&self.source_profile).unwrap_or(0)
    }

    /// RFC 0047 direct-self-call admission (Source Profile 0.13+ only).
    /// On success the call site's structural-decrease record joins
    /// `structural_evidence` (R5), carrying the param-first projection
    /// chain the kernel re-derives. On failure the historical MNE130 fires
    /// with wording that names the narrow exception. Older profiles never
    /// decide here: their pre-elaboration check owns every self-cycle
    /// exactly as before, so no diagnostic can double-report.
    fn check_self_call_structure(
        &mut self,
        signature: &FunctionSignature,
        first_argument: Option<&ResolvedBinding>,
        call_op: &str,
        span: SourceSpan,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) {
        if !self.profile_0_13() {
            return;
        }
        // R1: the recursive parameter (first) has finite type. Generic
        // parameters are never finite here; generic recursion stays
        // rejected until a design for it exists.
        let finite_first = matches!(signature.inputs.first(), Some(BodyType::Finite { .. }));
        let argument =
            first_argument.map(|resolved| (resolved.binding.clone(), resolved.id.clone()));
        let admitted = match (finite_first, self.measure.clone(), argument) {
            (
                true,
                Some((measure_binding, measure_value)),
                Some((Some(arg_binding), arg_value)),
            ) => self
                .descendant_chain(&measure_binding, &arg_binding, &arg_value)
                .map(|links| (measure_binding, measure_value, arg_binding, links)),
            _ => None,
        };
        match admitted {
            Some((param_binding, param_value, arg_binding, links)) => {
                self.structural_evidence
                    .push(mncs_model::StructuralDecreaseClaim {
                        call_op: call_op.to_owned(),
                        function: self.owner.clone(),
                        param_binding,
                        param_value,
                        arg_binding,
                        links,
                    });
            }
            None => diagnostics.push(elaboration_diagnostic(
                "MNE130",
                "recursive call cycles are rejected; only a direct self-call that consumes a match-bound structural descendant of a finite first parameter is admitted on Source Profile 0.13 or later (RFC 0047)",
                span,
            )),
        }
    }

    /// Walk resolved-binding provenance from the argument back to the
    /// measure (R2/R3), collecting param-first projection links. Returns
    /// `None` for a root call, an opaque argument (aliases, call results,
    /// rebuilt values, arithmetic, iteration state), a chain that leaves
    /// the measure's descent, or a chain that cannot close within the map
    /// (defensive fuel: provenance strictly descends binding-creation
    /// order, so honest chains always close).
    fn descendant_chain(
        &self,
        measure_binding: &SemanticId,
        arg_binding: &SemanticId,
        arg_value: &str,
    ) -> Option<Vec<mncs_model::StructuralDecreaseLink>> {
        let mut links = Vec::new();
        let mut current_binding = arg_binding.clone();
        let mut current_value = arg_value.to_owned();
        for _ in 0..self.recursion_provenance.len() + 1 {
            let provenance = self.recursion_provenance.get(&current_binding)?;
            match provenance {
                RecursionProvenance::Measure => {
                    if current_binding == *measure_binding && !links.is_empty() {
                        links.reverse();
                        return Some(links);
                    }
                    return None;
                }
                RecursionProvenance::Projected {
                    parent_binding,
                    parent_value,
                    op,
                    ..
                } => {
                    links.push(mncs_model::StructuralDecreaseLink {
                        op: op.clone(),
                        parent_binding: parent_binding.clone(),
                        child_binding: current_binding.clone(),
                        parent_value: parent_value.clone(),
                        child_value: current_value.clone(),
                    });
                    current_binding = parent_binding.clone();
                    current_value = parent_value.clone();
                }
            }
        }
        None
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
                    self.admitted_sequence_ceiling(),
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
                // Same-scope shadowing is a Profile 0.13 extension
                // (ENG-PRESSURE-0021). Older profiles keep the historical
                // MNE110 refusal for `let` over a plain value or parameter
                // in the same scope; rebinding over index/state names stays
                // MNE110 on every profile via `bind` below.
                if !self.profile_0_13() && env.shadows_plain_value_in_current_scope(&name.text) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE110",
                        "binding is ambiguous in this lexical scope",
                        name.span,
                    ));
                    return;
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
        // Profile 0.11 permits two levels of nested bounded iterations
        // (bit-steps within byte-steps, the natural checksum shape) with
        // distinct iteration identities; the per-loop bound (1..=32) keeps
        // the dynamic step product within the step budget. Older profiles
        // keep the historical refusal, and depth three or more stays
        // refused on every profile.
        if self.iteration_depth >= 1 && !self.profile_nested_iteration() {
            diagnostics.push(elaboration_diagnostic(
                "MNE147",
                "Source Profile 0.4 does not permit nested bounded iterations",
                *span,
            ));
            return;
        }
        if self.iteration_depth >= 2 {
            diagnostics.push(elaboration_diagnostic(
                "MNE147",
                "Source Profile 0.11 permits at most two levels of nested bounded iterations",
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
        let mut traversal_generic = false;
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
                traversal_generic = bound.is_generic();
                // u64 traversal domains are a Profile 0.13 extension: the
                // traversal index is an abstract u64 counter, and older
                // profiles confused a u64 *element* type with that counter
                // (historical MNB101 at model validation). The refusal now
                // fires here so model validation stays profile-agnostic;
                // MNE194 is the elaboration-stage counterpart naming the
                // profile rule.
                if !self.profile_0_13()
                    && **element
                        == BodyType::Integer(IntegerType {
                            bits: 64,
                            signed: false,
                        })
                {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE194",
                        "u64 sequence traversal domains require source profile 0.13 or later",
                        *source_span,
                    ));
                    return;
                }
                let ceiling = bound.ceiling();
                // Admitted sequence ceiling comes from profile policy (RFC
                // 0036): 64 through Profile 0.12, 1024 in 0.13. Generic
                // bounds defer the admitted check to instantiation: a
                // `[T; N]` traversal is admitted here against the absolute
                // model ceiling (which `Param` reports by construction) and
                // each concrete substitution is checked against the admitted
                // ceiling after specialization, so a generic definition stays
                // profile-portable while over-ceiling instantiations still
                // fail closed with MNE182.
                let admitted_sequence = if bound.is_generic() {
                    mncs_model::MODEL_MAX_SEQUENCE_BOUND
                } else {
                    mncs_syntax::max_sequence_bound_for(&self.source_profile).unwrap_or(0)
                };
                if ceiling > admitted_sequence {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE182",
                        format!(
                            "sequence traversal bound must not exceed the profile ceiling {admitted_sequence}",
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
                // Admitted per-level ceiling comes from profile policy (RFC
                // 0036): 1..=32 through Profile 0.12, 1..=1024 in 0.13.
                let ceiling =
                    mncs_syntax::max_iteration_bound_for(&self.source_profile).unwrap_or(0);
                if bound_u32 == 0 || bound_u32 > ceiling {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE142",
                        format!(
                            "iteration bound must be between 1 and {ceiling} in Source Profile {}",
                            self.source_profile
                        ),
                        bound.span,
                    ));
                    return;
                }
                bound_u32
            }
        };
        // Iteration identities (Profile 0.13, CP-0009): uniqueness is scoped
        // to the live lexical scope, so two sequential non-overlapping loops
        // may reuse a source-level index name while a nested loop reusing a
        // still-open enclosing identity stays rejected (MNE146). Older
        // profiles keep the historical function-wide rule: any reuse —
        // sequential or nested — is MNE146 with the historical message, and
        // recording stays verbatim. The recorded identity below is hygienic
        // (`name`, `name#2`, ...) so the proof graph, obligation subjects,
        // and MNB061 uniqueness stay per-loop even when the source name
        // repeats; only the source spelling is scoped here.
        if self.profile_0_13() {
            if self.open_iterations.iter().any(|open| open.0 == name.text) {
                diagnostics.push(elaboration_diagnostic(
                    "MNE146",
                    "iteration identity is already bound by an enclosing iteration",
                    name.span,
                ));
                return;
            }
        } else if self.open_iterations.iter().any(|open| open.0 == name.text)
            || self
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
        // Compositional bounded-work rule: the static product of the
        // enclosing bounds and this loop's bound must fit the profile's
        // admitted work envelope (registry). Through 0.12 the envelope
        // admits every historically accepted shape (counted 32s and
        // traversal-64 compositions); 0.13 admits an explicit 1024x1024
        // envelope. Calls inside the body can multiply work transitively;
        // that residual cost is an obligation on the callee's own bounds
        // (see the resource-limit architecture note), not part of this
        // static product.
        let enclosing_product: u64 = self
            .open_iterations
            .iter()
            .map(|(_, enclosing)| u64::from(*enclosing))
            .product();
        // Generic traversal bounds contribute the admitted sequence ceiling,
        // not the absolute model maximum, to the static work product: each
        // concrete substitution is independently capped by the
        // post-specialization sweep, so no instantiated product can exceed
        // the product computed here. Doubly-nested generic traversals may
        // fail closed here even when a small instantiation would fit;
        // recomputing the product at instantiation time is future work.
        let level_bound = if traversal_generic {
            mncs_syntax::max_sequence_bound_for(&self.source_profile).unwrap_or(0)
        } else {
            bound_u32
        };
        let work_product = enclosing_product.saturating_mul(u64::from(level_bound));
        let envelope =
            mncs_syntax::max_iteration_work_product_for(&self.source_profile).unwrap_or(0);
        if work_product > envelope {
            diagnostics.push(elaboration_diagnostic(
                "MNE193",
                format!(
                    "nested iteration work product {work_product} exceeds the Source Profile {} static envelope {envelope}",
                    self.source_profile
                ),
                *span,
            ));
            return;
        }
        self.open_iterations.push((name.text.clone(), level_bound));
        // Claim the hygienic recorded identity in elaboration (source)
        // order, so the mapping is deterministic. `#` never lexes inside a
        // name (MNL002), so a suffixed identity can never collide with a
        // user-written one.
        let prior_uses = self
            .iteration_name_uses
            .get(&name.text)
            .copied()
            .unwrap_or(0);
        self.iteration_name_uses
            .insert(name.text.clone(), prior_uses + 1);
        let recorded_id = if prior_uses == 0 {
            name.text.clone()
        } else {
            format!("{}#{}", name.text, prior_uses + 1)
        };
        let carried_type = profile_type_with_generics(
            &state_type.text,
            state_type.span,
            self.finite_types,
            self.record_types,
            &self.generic_map,
            diagnostics,
            self.admitted_sequence_ceiling(),
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
            arguments: vec![initial_value.id.clone(), bound_id.clone()],
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
        self.iteration_depth += 1;
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
        } else if self.profile_nested_iteration() {
            // Profile 0.11 binds the counted-loop index in the body scope:
            // the 0-based position `bound - remaining`, typed u64 like the
            // counter. Older profiles leave it unbound (MNE102). The
            // counted position proves nothing about sequence bounds, so it
            // binds as CountedIndex, never as TraversalIndex.
            let index_id = self.new_value("counted_index");
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
                operands: vec![bound_id.clone(), header_counter.clone()],
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
                BoundNameKind::CountedIndex,
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
        // The loop body is the only place a nested iteration can open (the
        // step clause is expression-only), so the identity closes here: a
        // later sequential loop may reuse the source name while any loop
        // still elaborating its body keeps MNE146. This single pop covers
        // every exit below uniformly.
        self.open_iterations.pop();
        if !self.block_is_open() {
            diagnostics.push(elaboration_diagnostic(
                "MNE148",
                "iteration continuation is unreachable; every body path terminated",
                next_value.span(),
            ));
            env.pop();
            self.iteration_depth -= 1;
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
            self.iteration_depth -= 1;
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
        self.iteration_depth -= 1;
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
            id: recorded_id,
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

    /// Elaborate `match` over a `bool` subject (HARNESS-PRESSURE-013).
    ///
    /// Boolean patterns carry the same exhaustiveness rule as a two-variant
    /// finite type: `true` and `false` must each appear exactly once.
    /// Anything else (qualified patterns, payload bindings, unknown names)
    /// fails with the same diagnostic codes as the finite-type path. The
    /// accepted form lowers to the branchless `Select` operation every
    /// backend already realizes, rather than growing a parallel
    /// boolean-dispatch form.
    fn elaborate_bool_match(
        &mut self,
        subject: &ResolvedBinding,
        arms: &[AstMatchArm],
        span: &SourceSpan,
        expected: Option<&BodyType>,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        let Some(result_type) = expected.cloned() else {
            diagnostics.push(elaboration_diagnostic(
                "MNE137",
                "match result requires an expected type in Source Profile 0.3",
                *span,
            ));
            return None;
        };
        let diagnostic_count = diagnostics.len();
        let mut true_arm: Option<&AstExpr> = None;
        let mut false_arm: Option<&AstExpr> = None;
        for arm in arms {
            // Scalar patterns need an integer subject (CP-0010); on a bool
            // they are not variants of a two-case domain.
            if !matches!(arm.pattern, AstMatchPattern::Variant) {
                diagnostics.push(elaboration_diagnostic(
                    "MNE138",
                    "scalar match patterns require an integer match subject",
                    arm.variant.span,
                ));
                continue;
            }
            if arm.type_name.is_some()
                || (arm.variant.text != "true" && arm.variant.text != "false")
            {
                diagnostics.push(elaboration_diagnostic(
                    "MNE138",
                    "match arm names a variant outside the subject's finite type",
                    arm.variant.span,
                ));
                continue;
            }
            if !arm.bindings.is_empty() || arm.ignore_payload {
                diagnostics.push(elaboration_diagnostic(
                    "MNE177",
                    "pattern binds a payload field the variant does not declare",
                    arm.variant.span,
                ));
                continue;
            }
            let slot = if arm.variant.text == "true" {
                &mut true_arm
            } else {
                &mut false_arm
            };
            if slot.is_some() {
                diagnostics.push(elaboration_diagnostic(
                    "MNE139",
                    "duplicate match arm is unreachable",
                    arm.variant.span,
                ));
                continue;
            }
            *slot = Some(&arm.value);
        }
        let missing = ["true", "false"]
            .into_iter()
            .filter(|name| {
                (*name == "true" && true_arm.is_none()) || (*name == "false" && false_arm.is_none())
            })
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
        if diagnostics.len() != diagnostic_count || true_arm.is_none() || false_arm.is_none() {
            return None;
        }
        let (Some(true_expr), Some(false_expr)) = (true_arm, false_arm) else {
            return None;
        };
        let true_binding = self.elaborate_expr(true_expr, Some(&result_type), env, diagnostics)?;
        if true_binding.ty != result_type {
            diagnostics.push(elaboration_diagnostic(
                "MNE141",
                "match arms must produce the same expected type",
                true_expr.span(),
            ));
            return None;
        }
        let false_binding =
            self.elaborate_expr(false_expr, Some(&result_type), env, diagnostics)?;
        if false_binding.ty != result_type {
            diagnostics.push(elaboration_diagnostic(
                "MNE141",
                "match arms must produce the same expected type",
                false_expr.span(),
            ));
            return None;
        }
        let id = self.new_value("boolsel");
        self.blocks[self.current].operations.push(BodyOperation {
            id: id.clone(),
            kind: BodyOperationKind::Select {
                operand_type: Box::new(result_type.clone()),
            },
            operands: vec![subject.id.clone(), true_binding.id, false_binding.id],
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

    /// Elaborate `match` over an integer subject (CP-0010).
    ///
    /// The contract is total dispatch over an open scalar domain:
    /// - literal arms cover exactly their literal value; duplicates are
    ///   rejected (MNE139, like duplicate variant arms);
    /// - exactly one `_` default arm is required (MNE140 when missing,
    ///   MNE139 when duplicated);
    /// - arms after the default are unreachable (MNE139);
    /// - literals are range-checked against the subject type at
    ///   elaboration with no silent truncation (MNE145);
    /// - a bare `_` (no qualifier, no payload) is the default arm. On
    ///   finite/bool subjects the same spelling keeps its historical
    ///   variant meaning, so only the integer path interprets it;
    /// - anything else (variant names, `true`/`false`, qualified patterns,
    ///   payload bindings) is rejected (MNE138).
    ///
    /// Lowering reuses the finite-match branch-chain shape: one
    /// `IntegerCompare eq` test per literal arm in source order with the
    /// default as the terminal unconditional branch. Exhaustiveness is
    /// decided here, never in a backend: every backend already realizes
    /// branches, integer comparison, and constants.
    #[allow(clippy::too_many_arguments)]
    fn elaborate_scalar_match(
        &mut self,
        subject: &ResolvedBinding,
        operand_type: IntegerType,
        arms: &[AstMatchArm],
        span: &SourceSpan,
        expected: Option<&BodyType>,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        let Some(result_type) = expected.cloned() else {
            diagnostics.push(elaboration_diagnostic(
                "MNE137",
                "match result requires an expected type in Source Profile 0.3",
                *span,
            ));
            return None;
        };
        let (range_min, range_max) = integer_range(operand_type);
        let diagnostic_count = diagnostics.len();
        // Resolved arms in source order: `Some(literal)` for a literal arm,
        // `None` for the default.
        let mut resolved_arms: Vec<(Option<i128>, &AstExpr, SourceSpan)> = Vec::new();
        let mut seen = BTreeSet::new();
        let mut default_seen = false;
        for arm in arms {
            match &arm.pattern {
                AstMatchPattern::Scalar { negative, text } => {
                    let Some(literal) = scalar_pattern_value(*negative, text, diagnostics) else {
                        continue;
                    };
                    if literal < range_min || literal > range_max {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE145",
                            format!(
                                "scalar match literal {literal} is outside the subject integer type ({}..={})",
                                range_min, range_max
                            ),
                            text.span,
                        ));
                        continue;
                    }
                    if default_seen {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE139",
                            "duplicate match arm is unreachable",
                            arm.variant.span,
                        ));
                        continue;
                    }
                    if !seen.insert(literal) {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE139",
                            "duplicate match arm is unreachable",
                            arm.variant.span,
                        ));
                        continue;
                    }
                    resolved_arms.push((Some(literal), &arm.value, arm.variant.span));
                }
                AstMatchPattern::Variant => {
                    let is_default = arm.type_name.is_none()
                        && arm.variant.text == "_"
                        && arm.bindings.is_empty()
                        && !arm.ignore_payload;
                    if !is_default {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE138",
                            "scalar match arms must be integer literals or `_`",
                            arm.variant.span,
                        ));
                        continue;
                    }
                    if default_seen {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE139",
                            "duplicate match arm is unreachable",
                            arm.variant.span,
                        ));
                        continue;
                    }
                    default_seen = true;
                    resolved_arms.push((None, &arm.value, arm.variant.span));
                }
            }
        }
        if !default_seen {
            diagnostics.push(elaboration_diagnostic(
                "MNE140",
                "non-exhaustive scalar match; a wildcard arm `_` is required",
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
        for (index, ((literal, _, _), arm_id)) in resolved_arms.iter().zip(&arm_ids).enumerate() {
            self.current = dispatch;
            if index + 1 == resolved_arms.len() {
                self.blocks[self.current].terminator = BodyTerminator::Branch {
                    target: arm_id.clone(),
                    arguments: Vec::new(),
                };
            } else {
                let literal = literal.expect("non-terminal scalar arm covers a literal");
                let constant = self.new_value("matchlit");
                self.blocks[self.current].operations.push(BodyOperation {
                    id: constant.clone(),
                    kind: BodyOperationKind::Constant {
                        value: literal,
                        ty: BodyType::Integer(operand_type),
                    },
                    operands: Vec::new(),
                    results: vec![BodyValue {
                        id: constant.clone(),
                        ty: BodyType::Integer(operand_type),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                });
                let condition = self.new_value("match");
                self.blocks[self.current].operations.push(BodyOperation {
                    id: condition.clone(),
                    kind: BodyOperationKind::IntegerCompare {
                        predicate: "eq".to_owned(),
                        operand_type,
                    },
                    operands: vec![subject.id.clone(), constant],
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
        for ((_, arm_expr, _), arm_id) in resolved_arms.iter().zip(&arm_ids) {
            self.current = self.index_of(arm_id);
            env.push();
            if let Some(value) = self.elaborate_expr(arm_expr, Some(&result_type), env, diagnostics)
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

    /// Elaborate the `host_read()` intrinsic (HARNESS-PRESSURE-004).
    ///
    /// Check the declared host authority for one host intrinsic
    /// (HARNESS-PRESSURE-004/005/006). Returns the authorizing capability
    /// when the enclosing function declares exactly one effect of
    /// `effect_kind` plus that capability; otherwise emits the intrinsic's
    /// missing/double diagnostics and returns `None`. Argument-carrying
    /// intrinsics (`sha256_digest`, `ed25519_verify`) still take data
    /// operands — data is not authority — but the capability record that
    /// authorizes realization comes only from here.
    #[allow(clippy::too_many_arguments)]
    fn check_host_authority(
        &self,
        effect_kind: &str,
        missing_code: &str,
        double_code: &str,
        span: SourceSpan,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<String> {
        let signature = self.signatures.get(&self.function);
        let mut granted: Vec<&Effect> = signature
            .into_iter()
            .flat_map(|signature| signature.effects.iter())
            .filter(|effect| effect.kind == effect_kind)
            .collect();
        if granted.is_empty() {
            diagnostics.push(elaboration_diagnostic(
                missing_code,
                format!(
                    "host intrinsic requires a declared {effect_kind} effect with its authorizing capability"
                ),
                span,
            ));
            return None;
        }
        if granted.len() > 1 {
            diagnostics.push(elaboration_diagnostic(
                double_code,
                format!(
                    "host intrinsic requires exactly one declared {effect_kind} effect per function"
                ),
                span,
            ));
            return None;
        }
        let granted = granted.pop().expect("one host effect");
        let capabilities = signature
            .map(|signature| signature.capabilities.clone())
            .unwrap_or_default();
        if !capabilities.contains(&granted.capability) {
            diagnostics.push(elaboration_diagnostic(
                missing_code,
                format!(
                    "host intrinsic requires a declared {effect_kind} effect with its authorizing capability"
                ),
                span,
            ));
            return None;
        }
        Some(granted.capability.clone())
    }

    /// Elaborate the `clock_read()` intrinsic (HARNESS-PRESSURE-005).
    ///
    /// Authority comes entirely from the enclosing function's
    /// declarations: exactly one `clock_read` effect plus its authorizing
    /// capability. The value (epoch milliseconds as `u64`) is realized by
    /// the executor from its own clock once the operator grants that
    /// capability (`--grant-time`); no grant file backs it, so programs
    /// must compare instants relationally, never pin absolute values.
    fn elaborate_clock_read(
        &mut self,
        span: SourceSpan,
        expected: Option<&BodyType>,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        let result_ty = BodyType::Integer(IntegerType {
            bits: 64,
            signed: false,
        });
        if expected.is_some_and(|expected| expected != &result_ty) {
            diagnostics.push(elaboration_diagnostic(
                "MNE240",
                format!(
                    "clock_read produces {} which does not satisfy the required type",
                    result_ty.semantic_name()
                ),
                span,
            ));
            return None;
        }
        let capability =
            self.check_host_authority("clock_read", "MNE238", "MNE239", span, diagnostics)?;
        let id = self.new_value("clockread");
        self.blocks[self.current].operations.push(BodyOperation {
            id: id.clone(),
            kind: BodyOperationKind::HostCall {
                capability,
                operation: "clock_read".to_owned(),
            },
            operands: Vec::new(),
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

    /// Authority comes entirely from the enclosing function's
    /// declarations: exactly one `host_read` effect plus its authorizing
    /// capability. The value (a `[byte; up_to 64]` view) is realized by the
    /// executor from an explicit grant for that capability; there is no
    /// ambient source and no argument that could smuggle one in.
    fn elaborate_host_read(
        &mut self,
        span: SourceSpan,
        expected: Option<&BodyType>,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        let result_ty = BodyType::Sequence {
            element: Box::new(BodyType::Byte),
            bound: mncs_model::SequenceBound::UpTo(64),
        };
        if expected.is_some_and(|expected| expected != &result_ty) {
            diagnostics.push(elaboration_diagnostic(
                "MNE237",
                format!(
                    "host_read produces {} which does not satisfy the required type",
                    result_ty.semantic_name()
                ),
                span,
            ));
            return None;
        }
        let capability =
            self.check_host_authority("host_read", "MNE235", "MNE236", span, diagnostics)?;
        let id = self.new_value("hostread");
        self.blocks[self.current].operations.push(BodyOperation {
            id: id.clone(),
            kind: BodyOperationKind::HostCall {
                capability,
                operation: "blob_read".to_owned(),
            },
            operands: Vec::new(),
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

    /// Elaborate the nullary granted-filesystem intrinsics
    /// (`fs_list_count`, `fs_generation`; index PRESS-003). Both produce
    /// `u64` under exactly one declared `fs_list` effect plus its
    /// authorizing capability (`--grant-fs`); indices and paths never
    /// appear in source, so there are no operands to check.
    fn elaborate_fs_nullary(
        &mut self,
        operation: &str,
        effect_kind: &str,
        span: SourceSpan,
        expected: Option<&BodyType>,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        let result_ty = BodyType::Integer(IntegerType {
            bits: 64,
            signed: false,
        });
        if expected.is_some_and(|expected| expected != &result_ty) {
            diagnostics.push(elaboration_diagnostic(
                "MNE261",
                format!(
                    "{operation} produces {} which does not satisfy the required type",
                    result_ty.semantic_name()
                ),
                span,
            ));
            return None;
        }
        let capability =
            self.check_host_authority(effect_kind, "MNE257", "MNE258", span, diagnostics)?;
        let id = self.new_value("fslist");
        self.blocks[self.current].operations.push(BodyOperation {
            id: id.clone(),
            kind: BodyOperationKind::HostCall {
                capability,
                operation: operation.to_owned(),
            },
            operands: Vec::new(),
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

    /// Elaborate one u64 index operand of a filesystem intrinsic. Indices
    /// are data, never authority: a stale or wild index fails the call
    /// closed at realization (InvalidRequest), never with a value.
    fn elaborate_fs_index(
        &mut self,
        operation: &str,
        index: &AstExpr,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        let counter_type = BodyType::Integer(IntegerType {
            bits: 64,
            signed: false,
        });
        let binding = self.elaborate_expr(index, Some(&counter_type), env, diagnostics)?;
        if binding.ty != counter_type {
            diagnostics.push(elaboration_diagnostic(
                "MNE262",
                format!("{operation} index/offset/length operands must have u64 type"),
                index.span(),
            ));
            return None;
        }
        Some(binding)
    }

    /// Elaborate the indexed granted-filesystem intrinsics
    /// (`fs_entry_name_at`, `fs_entry_kind_at`; index PRESS-003) under
    /// exactly one declared `fs_list` effect. Names deliver
    /// `[byte; up_to 64]`; kinds deliver `u64` (0 = file, 1 = dir,
    /// 2 = other).
    fn elaborate_fs_entry_at(
        &mut self,
        operation: &str,
        index: &AstExpr,
        span: SourceSpan,
        expected: Option<&BodyType>,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        let result_ty = if operation == "fs_entry_name_at" {
            BodyType::Sequence {
                element: Box::new(BodyType::Byte),
                bound: mncs_model::SequenceBound::UpTo(64),
            }
        } else {
            BodyType::Integer(IntegerType {
                bits: 64,
                signed: false,
            })
        };
        if expected.is_some_and(|expected| expected != &result_ty) {
            diagnostics.push(elaboration_diagnostic(
                "MNE261",
                format!(
                    "{operation} produces {} which does not satisfy the required type",
                    result_ty.semantic_name()
                ),
                span,
            ));
            return None;
        }
        let capability =
            self.check_host_authority("fs_list", "MNE257", "MNE258", span, diagnostics)?;
        let operand = self.elaborate_fs_index(operation, index, env, diagnostics)?;
        let id = self.new_value("fsentry");
        self.blocks[self.current].operations.push(BodyOperation {
            id: id.clone(),
            kind: BodyOperationKind::HostCall {
                capability,
                operation: operation.to_owned(),
            },
            operands: vec![operand.id.clone()],
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

    /// Elaborate the chunked granted-filesystem read
    /// (`fs_read_bytes_at(entry, offset, length)`; index PRESS-003)
    /// under exactly one declared `fs_read` effect. Delivers up to 64
    /// bytes as `[byte; up_to 64]`; short reads and empty views at
    /// end-of-input are the EOF signal.
    fn elaborate_fs_read_bytes_at(
        &mut self,
        parts: (&AstExpr, &AstExpr, &AstExpr),
        span: SourceSpan,
        expected: Option<&BodyType>,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        let (entry, offset, length) = parts;
        let result_ty = BodyType::Sequence {
            element: Box::new(BodyType::Byte),
            bound: mncs_model::SequenceBound::UpTo(64),
        };
        if expected.is_some_and(|expected| expected != &result_ty) {
            diagnostics.push(elaboration_diagnostic(
                "MNE261",
                format!(
                    "fs_read_bytes_at produces {} which does not satisfy the required type",
                    result_ty.semantic_name()
                ),
                span,
            ));
            return None;
        }
        let capability =
            self.check_host_authority("fs_read", "MNE259", "MNE260", span, diagnostics)?;
        let entry_binding = self.elaborate_fs_index("fs_read_bytes_at", entry, env, diagnostics)?;
        let offset_binding =
            self.elaborate_fs_index("fs_read_bytes_at", offset, env, diagnostics)?;
        let length_binding =
            self.elaborate_fs_index("fs_read_bytes_at", length, env, diagnostics)?;
        let id = self.new_value("fsread");
        self.blocks[self.current].operations.push(BodyOperation {
            id: id.clone(),
            kind: BodyOperationKind::HostCall {
                capability,
                operation: "fs_read_bytes_at".to_owned(),
            },
            operands: vec![
                entry_binding.id.clone(),
                offset_binding.id.clone(),
                length_binding.id.clone(),
            ],
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

    /// Elaborate one byte-view operand of a verify-only crypto intrinsic.
    /// The operand must elaborate to a byte sequence; coverage is the
    /// view's runtime bytes, so no length accompanies it. Fixed
    /// algorithm sizes (32-byte keys, 64-byte signatures) are enforced
    /// at realization, not here.
    fn elaborate_crypto_view(
        &mut self,
        view: &AstExpr,
        intrinsic: &str,
        type_code: &str,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        let binding = self.elaborate_expr(view, None, env, diagnostics)?;
        let BodyType::Sequence { element, .. } = binding.ty.clone() else {
            diagnostics.push(elaboration_diagnostic(
                type_code,
                format!("{intrinsic} requires byte-view operands, not a non-sequence value"),
                view.span(),
            ));
            return None;
        };
        if *element != BodyType::Byte {
            diagnostics.push(elaboration_diagnostic(
                type_code,
                format!("{intrinsic} requires byte-view operands over `byte` elements"),
                view.span(),
            ));
            return None;
        }
        Some(binding)
    }

    /// Elaborate the `sha256_digest(view)` intrinsic
    /// (HARNESS-PRESSURE-006). Verify-only: no secrets enter, the 32
    /// digest bytes leave as `[byte; up_to 64]`, and realization is a
    /// pure function of the operand through the audited SHA-256
    /// primitive. Authority is the declared `sha256_digest` effect plus
    /// its capability (`--grant-crypto`); the operand carries data only.
    fn elaborate_sha256_digest(
        &mut self,
        view: &AstExpr,
        span: SourceSpan,
        expected: Option<&BodyType>,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        let result_ty = BodyType::Sequence {
            element: Box::new(BodyType::Byte),
            bound: mncs_model::SequenceBound::UpTo(64),
        };
        if expected.is_some_and(|expected| expected != &result_ty) {
            diagnostics.push(elaboration_diagnostic(
                "MNE243",
                format!(
                    "sha256_digest produces {} which does not satisfy the required type",
                    result_ty.semantic_name()
                ),
                span,
            ));
            return None;
        }
        let capability =
            self.check_host_authority("sha256_digest", "MNE241", "MNE242", span, diagnostics)?;
        let operand =
            self.elaborate_crypto_view(view, "sha256_digest", "MNE243", env, diagnostics)?;
        let id = self.new_value("sha256");
        self.blocks[self.current].operations.push(BodyOperation {
            id: id.clone(),
            kind: BodyOperationKind::HostCall {
                capability,
                operation: "sha256_digest".to_owned(),
            },
            operands: vec![operand.id.clone()],
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

    /// Elaborate the `host_write(view)` intrinsic (P-006 storage slice).
    /// Bounded append-only storage through the host-capability boundary:
    /// the executor appends exactly the view's runtime bytes (at most 64
    /// per call) to the operator-granted path and returns the appended
    /// count as `u64`. Authority is the declared `host_write` effect plus
    /// its capability (`--grant-write`); the operand carries data only.
    /// There is no read-back, no truncation, and no ambient path: without
    /// a grant the call fails closed at realization.
    fn elaborate_host_write(
        &mut self,
        view: &AstExpr,
        span: SourceSpan,
        expected: Option<&BodyType>,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        let result_ty = BodyType::Integer(IntegerType {
            bits: 64,
            signed: false,
        });
        if expected.is_some_and(|expected| expected != &result_ty) {
            diagnostics.push(elaboration_diagnostic(
                "MNE255",
                format!(
                    "host_write produces {} which does not satisfy the required type",
                    result_ty.semantic_name()
                ),
                span,
            ));
            return None;
        }
        let capability =
            self.check_host_authority("host_write", "MNE253", "MNE254", span, diagnostics)?;
        let operand = self.elaborate_crypto_view(view, "host_write", "MNE255", env, diagnostics)?;
        let id = self.new_value("hostwrite");
        self.blocks[self.current].operations.push(BodyOperation {
            id: id.clone(),
            kind: BodyOperationKind::HostCall {
                capability,
                operation: "blob_append".to_owned(),
            },
            operands: vec![operand.id.clone()],
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

    /// Elaborate the `sin(x)` / `cos(x)` float intrinsics (Profile
    /// 0.12). The operand and result are binary64; a non-float operand
    /// is refused, and the non-finite trap obligation is recorded with
    /// the operation like arithmetic.
    fn elaborate_float_intrinsic(
        &mut self,
        name: &str,
        argument: &AstExpr,
        span: SourceSpan,
        expected: Option<&BodyType>,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        if !matches!(name, "sin" | "cos") {
            diagnostics.push(elaboration_diagnostic(
                "MNE250",
                format!("unsupported float intrinsic {name:?}"),
                span,
            ));
            return None;
        }
        if !self.profile_float() {
            diagnostics.push(elaboration_diagnostic(
                "MNE247",
                "float values require source profile 0.12 or later",
                span,
            ));
            return None;
        }
        let float_ty = BodyType::Float(mncs_model::FloatType::f64());
        if expected.is_some_and(|expected| expected != &float_ty) {
            diagnostics.push(elaboration_diagnostic(
                "MNE252",
                format!(
                    "float intrinsic {name} produces {} which does not satisfy the required type",
                    float_ty.semantic_name()
                ),
                span,
            ));
            return None;
        }
        let operand = self.elaborate_expr(argument, Some(&float_ty), env, diagnostics)?;
        if operand.ty != float_ty {
            diagnostics.push(elaboration_diagnostic(
                "MNE251",
                format!("float intrinsic {name} requires a binary64 operand"),
                argument.span(),
            ));
            return None;
        }
        let id = self.new_value("fi");
        self.blocks[self.current].operations.push(BodyOperation {
            id: id.clone(),
            kind: BodyOperationKind::FloatIntrinsic {
                function: name.to_owned(),
            },
            operands: vec![operand.id.clone()],
            results: vec![BodyValue {
                id: id.clone(),
                ty: float_ty.clone(),
            }],
            contracts: Vec::new(),
            assumptions: Vec::new(),
            machine_intent: None,
            lowering: None,
            portability: None,
        });
        Some(ResolvedBinding::plain(id, float_ty))
    }

    /// Elaborate the `ed25519_verify(pubkey, message, signature)`
    /// intrinsic (HARNESS-PRESSURE-006). Verify-only: no keygen exists
    /// in-language, and realization reports a boolean through the
    /// audited dalek primitive. Authority is the declared
    /// `ed25519_verify` effect plus its capability (`--grant-crypto`).
    /// Eight parameters because the three operand expressions, the
    /// expected type, and the elaboration context each travel
    /// separately; bundling them would obscure the call sites.
    #[allow(clippy::too_many_arguments)]
    fn elaborate_ed25519_verify(
        &mut self,
        pubkey: &AstExpr,
        message: &AstExpr,
        signature: &AstExpr,
        span: SourceSpan,
        expected: Option<&BodyType>,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        let result_ty = BodyType::Named("bool".to_owned());
        if expected.is_some_and(|expected| expected != &result_ty) {
            diagnostics.push(elaboration_diagnostic(
                "MNE246",
                format!(
                    "ed25519_verify produces {} which does not satisfy the required type",
                    result_ty.semantic_name()
                ),
                span,
            ));
            return None;
        }
        let capability =
            self.check_host_authority("ed25519_verify", "MNE244", "MNE245", span, diagnostics)?;
        let key =
            self.elaborate_crypto_view(pubkey, "ed25519_verify", "MNE246", env, diagnostics)?;
        let msg =
            self.elaborate_crypto_view(message, "ed25519_verify", "MNE246", env, diagnostics)?;
        let sig =
            self.elaborate_crypto_view(signature, "ed25519_verify", "MNE246", env, diagnostics)?;
        let id = self.new_value("edverify");
        self.blocks[self.current].operations.push(BodyOperation {
            id: id.clone(),
            kind: BodyOperationKind::HostCall {
                capability,
                operation: "ed25519_verify".to_owned(),
            },
            operands: vec![key.id.clone(), msg.id.clone(), sig.id.clone()],
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
                if let Some(expected_ty) = expected {
                    if expected_ty != &resolved.ty {
                        if let Some(borrowed) =
                            self.borrow_view_for_expected(&resolved, expected_ty)
                        {
                            return Some(borrowed);
                        }
                        diagnostics.push(elaboration_diagnostic(
                            "MNE117",
                            "resolved name does not have the required expression type",
                            name.span,
                        ));
                    }
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
            AstExpr::Float { bits, text } => {
                if !self.profile_float() {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE247",
                        "float values require source profile 0.12 or later",
                        text.span,
                    ));
                    return None;
                }
                let ty = match expected {
                    Some(BodyType::Float(float)) if float.is_supported() => BodyType::Float(*float),
                    Some(_) => {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE118",
                            "float literal cannot satisfy a non-float type",
                            text.span,
                        ));
                        BodyType::Float(FloatType::f64())
                    }
                    None => BodyType::Float(FloatType::f64()),
                };
                let id = self.new_value("fc");
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind: BodyOperationKind::FloatConstant {
                        bits: *bits,
                        ty: FloatType::f64(),
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
                    // A two-segment `alias.Record { ... }` parses as
                    // FiniteVariant, so a qualified record constructor lands
                    // here. Retry the joined spelling in the record
                    // namespace before failing: finite constructors keep
                    // priority (behavior for valid programs is unchanged),
                    // and only a previously-rejected spelling can resolve.
                    let joined = format!("{}.{}", type_name.text, variant.text);
                    if self.record_types.contains_key(&joined) {
                        let literal = AstExpr::RecordLiteral {
                            type_name: SpannedText {
                                text: joined,
                                span: *span,
                            },
                            base: None,
                            fields: fields.clone(),
                            span: *span,
                        };
                        return self.elaborate_expr(&literal, expected, env, diagnostics);
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
                        self.admitted_sequence_ceiling(),
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
                            // Deterministic inference (Profile 0.13,
                            // ENG-PRESSURE-0019): solve directly-constrained
                            // parameters from the value arguments; anything
                            // ambiguous keeps the explicit-argument
                            // diagnostic with the unresolved names spelled
                            // out. Older profiles keep the historical
                            // refusal: inference is not available there.
                            if !self.profile_0_13() {
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
                            match self.infer_generic_args(&signature, arguments, env, diagnostics) {
                                Ok(inferred) => inferred,
                                Err(GenericInferenceFailure::ArgError) => return None,
                                Err(GenericInferenceFailure::Arity) => {
                                    diagnostics.push(elaboration_diagnostic(
                                        "MNE220",
                                        format!(
                                            "generic function '{}' requires {} generic argument(s); supply explicit <...>",
                                            function.text,
                                            callee_params.len()
                                        ),
                                        function.span,
                                    ));
                                    return None;
                                }
                                Err(GenericInferenceFailure::Ambiguous {
                                    missing,
                                    conflicting,
                                }) => {
                                    let mut reasons = Vec::new();
                                    if !missing.is_empty() {
                                        reasons
                                            .push(format!("cannot infer {}", missing.join(", ")));
                                    }
                                    if !conflicting.is_empty() {
                                        reasons.push(format!(
                                            "conflicting arguments for {}",
                                            conflicting.join(", ")
                                        ));
                                    }
                                    diagnostics.push(elaboration_diagnostic(
                                        "MNE220",
                                        format!(
                                            "generic function '{}' requires {} generic argument(s); {}; supply explicit <...>",
                                            function.text,
                                            callee_params.len(),
                                            reasons.join("; ")
                                        ),
                                        function.span,
                                    ));
                                    return None;
                                }
                            }
                        } else {
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
                                            self.admitted_sequence_ceiling(),
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
                                            let admitted = self.admitted_sequence_ceiling();
                                            if val > admitted {
                                                diagnostics.push(elaboration_diagnostic(
                                                "MNE225",
                                                format!(
                                                    "sequence bound value {val} exceeds profile ceiling {admitted}",
                                                ),
                                                arg.text.span,
                                            ));
                                                return None;
                                            }
                                            args_out
                                                .push(mncs_model::GenericArg::Value { value: val });
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
                // The elaborated first argument (RFC 0047): a direct
                // self-call is admitted only when this value resolves to a
                // strict structural descendant of the recursive parameter.
                // A borrowed operand carries the borrow's own binding, which
                // is never a match descendant, so borrowed first arguments
                // stay rejected exactly like any other opaque shape.
                let mut first_argument: Option<ResolvedBinding> = None;
                for (index, (source_argument, parameter_type)) in
                    arguments.iter().zip(&concrete_inputs).enumerate()
                {
                    let argument = self.elaborate_expr(
                        source_argument,
                        Some(parameter_type),
                        env,
                        diagnostics,
                    )?;
                    if &argument.ty != parameter_type {
                        // Exact-to-bounded-view borrow backstop for argument
                        // shapes that do not thread the callee expectation
                        // (names and call results borrow at their own
                        // elaboration sites above).
                        if let Some(borrowed) =
                            self.borrow_view_for_expected(&argument, parameter_type)
                        {
                            operands.push(borrowed.id.clone());
                            if index == 0 {
                                first_argument = Some(borrowed);
                            }
                            continue;
                        }
                        diagnostics.push(elaboration_diagnostic(
                            "MNE133",
                            "call argument type does not match the callee parameter",
                            source_argument.span(),
                        ));
                    }
                    if index == 0 {
                        first_argument = Some(argument.clone());
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
                    let borrows = expected.is_some_and(|expected| {
                        exact_view_borrow_dimensions(&concrete_output, expected).is_some()
                    });
                    if !borrows {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE135",
                            "call result does not have the required expression type",
                            *span,
                        ));
                    }
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
                // Direct self-calls reach the cycle check here, with resolved
                // bindings available (RFC 0047). Older profiles stay silent:
                // their pre-elaboration check owns every self-cycle exactly
                // as before. On 0.13+ a self-call is admitted only with a
                // re-derivable structural decrease; anything else is MNE130.
                if signature.identity == self.owner {
                    self.check_self_call_structure(
                        &signature,
                        first_argument.as_ref(),
                        &id,
                        *span,
                        diagnostics,
                    );
                }
                let binding = ResolvedBinding::plain(id, concrete_output.clone());
                if let Some(expected_ty) = expected {
                    if expected_ty != &binding.ty {
                        if let Some(borrowed) = self.borrow_view_for_expected(&binding, expected_ty)
                        {
                            return Some(borrowed);
                        }
                    }
                }
                Some(binding)
            }
            AstExpr::Match { value, arms, span } => {
                let subject = self.elaborate_expr(value, None, env, diagnostics)?;
                // Boolean patterns (HARNESS-PRESSURE-013): `match` over a
                // `bool` subject accepts `true`/`false` arms with the same
                // exhaustiveness rule as a two-variant finite type.
                if subject.ty == BodyType::Named("bool".to_owned()) {
                    return self.elaborate_bool_match(
                        &subject,
                        arms,
                        span,
                        expected,
                        env,
                        diagnostics,
                    );
                }
                // Scalar integer match (Profile 0.13, CP-0010): `match`
                // over an integer subject accepts integer literal arms plus
                // one required `_` default. Older profiles keep the
                // historical refusal below (MNE136): an integer subject is
                // not a declared finite type there.
                if self.profile_0_13() {
                    if let BodyType::Integer(operand_type) = &subject.ty {
                        return self.elaborate_scalar_match(
                            &subject,
                            *operand_type,
                            arms,
                            span,
                            expected,
                            env,
                            diagnostics,
                        );
                    }
                }
                let BodyType::Finite {
                    identity: type_identity,
                    name: type_name,
                } = &subject.ty
                else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE136",
                        if self.profile_0_13() {
                            "match subject must have a declared finite, bool, or integer type"
                        } else {
                            "match subject must have a declared finite type"
                        },
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
                    // Scalar patterns need an integer subject (CP-0010);
                    // they never name a variant of a finite type.
                    if !matches!(arm.pattern, AstMatchPattern::Variant) {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE138",
                            "scalar match patterns require an integer match subject",
                            arm.variant.span,
                        ));
                        continue;
                    }
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
                            self.admitted_sequence_ceiling(),
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
                        let child_binding = env.bind_projection(
                            binding_name.text.clone(),
                            projected.clone(),
                            expected_field.clone(),
                            binding_name.span,
                            subject.binding.clone(),
                            diagnostics,
                        );
                        // RFC 0047 descendant provenance over resolved
                        // bindings: a payload of a name carrying provenance
                        // extends the chain; a payload of an opaque subject
                        // (call results, aliases, rebuilt values) records
                        // nothing and can never justify a recursive call.
                        if let (Some(child), Some(parent)) =
                            (child_binding, subject.binding.clone())
                        {
                            self.recursion_provenance.insert(
                                child,
                                RecursionProvenance::Projected {
                                    parent_binding: parent,
                                    parent_value: subject.id.clone(),
                                    op: projected.clone(),
                                },
                            );
                        }
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
                            self.admitted_sequence_ceiling(),
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
                    self.admitted_sequence_ceiling(),
                );
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
                // WEB-P-002: a projected exact-sequence field borrows as a
                // bounded view exactly like a named value does (same
                // N <= M, same-element rule, same borrow machinery); only
                // genuinely mismatched expectations stay MNE163.
                if expected.is_some_and(|expected| expected != &result_ty) {
                    if let Some(wanted) = expected {
                        if let Some(borrowed) = self.borrow_view_for_expected(&projected, wanted) {
                            return Some(borrowed);
                        }
                    }
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
            AstExpr::SequenceCopy {
                destination,
                dst_at,
                source,
                src_at,
                len,
                span,
            } => self.elaborate_sequence_copy(
                destination,
                dst_at,
                source,
                src_at,
                len,
                *span,
                expected,
                env,
                diagnostics,
            ),
            AstExpr::CheckedIndex {
                sequence,
                index,
                span,
            } => self.elaborate_checked_index(sequence, index, *span, expected, env, diagnostics),
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
            AstExpr::HostRead { span } => self.elaborate_host_read(*span, expected, diagnostics),
            AstExpr::FsListCount { span } => {
                self.elaborate_fs_nullary("fs_list_count", "fs_list", *span, expected, diagnostics)
            }
            AstExpr::FsGeneration { span } => {
                self.elaborate_fs_nullary("fs_generation", "fs_list", *span, expected, diagnostics)
            }
            AstExpr::FsEntryNameAt { index, span } => self.elaborate_fs_entry_at(
                "fs_entry_name_at",
                index,
                *span,
                expected,
                env,
                diagnostics,
            ),
            AstExpr::FsEntryKindAt { index, span } => self.elaborate_fs_entry_at(
                "fs_entry_kind_at",
                index,
                *span,
                expected,
                env,
                diagnostics,
            ),
            AstExpr::FsReadBytesAt {
                entry,
                offset,
                length,
                span,
            } => self.elaborate_fs_read_bytes_at(
                (entry, offset, length),
                *span,
                expected,
                env,
                diagnostics,
            ),
            AstExpr::ClockRead { span } => self.elaborate_clock_read(*span, expected, diagnostics),
            AstExpr::Sha256Digest { view, span } => {
                self.elaborate_sha256_digest(view, *span, expected, env, diagnostics)
            }
            AstExpr::HostWrite { view, span } => {
                self.elaborate_host_write(view, *span, expected, env, diagnostics)
            }
            AstExpr::FloatIntrinsic {
                name,
                argument,
                span,
            } => self.elaborate_float_intrinsic(
                &name.text,
                argument,
                *span,
                expected,
                env,
                diagnostics,
            ),
            AstExpr::Ed25519Verify {
                pubkey,
                message,
                signature,
                span,
            } => self.elaborate_ed25519_verify(
                pubkey,
                message,
                signature,
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
            AstExpr::SequenceRepeat {
                element,
                count,
                span,
            } => {
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
                // ENG-PRESSURE-0020: `[value; N]` elaborates once and
                // duplicates the operand, so element evaluation happens a
                // single time (the `Copy`-repeat semantics of Rust's
                // `[value; N]`). Length agreement with the expected exact
                // bound is checked statically; the declared bound itself
                // already passed the sequence-length ceiling.
                let parsed: u32 = count.text.parse().map_or_else(
                    |_| {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE256",
                            "repeat count is not a valid sequence length",
                            count.span,
                        ));
                        None
                    },
                    Some,
                )?;
                if parsed != length {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE184",
                        format!(
                            "repeat count {parsed} does not match the declared exact length {length}"
                        ),
                        *span,
                    ));
                    return None;
                }
                let resolved =
                    self.elaborate_expr(element, Some(&element_type), env, diagnostics)?;
                if &resolved.ty != element_type.as_ref() {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE185",
                        "sequence element value does not match the declared element type",
                        element.span(),
                    ));
                }
                let id = self.new_value("seq");
                let expected_ty = expected
                    .cloned()
                    .unwrap_or_else(|| BodyType::Named("invalid".to_owned()));
                self.blocks[self.current].operations.push(BodyOperation {
                    id: id.clone(),
                    kind: BodyOperationKind::SequenceConstruct {
                        element_type,
                        length,
                    },
                    operands: vec![resolved.id; parsed as usize],
                    results: vec![BodyValue {
                        id: id.clone(),
                        ty: expected_ty.clone(),
                    }],
                    contracts: Vec::new(),
                    assumptions: Vec::new(),
                    machine_intent: None,
                    lowering: None,
                    portability: None,
                });
                Some(ResolvedBinding::plain(id, expected_ty))
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
                    // A use of the exact value a dominating BoundCheck
                    // verified against this same sequence value discharges
                    // through the retained check; anything else keeps an
                    // explicit runtime check.
                    _ if self
                        .checked_indices
                        .get(&index_value.id)
                        .is_some_and(|checked| *checked == subject.id) =>
                    {
                        mncs_model::BoundsEvidence::CheckedBound
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
                if let Some(wanted) = expected {
                    if wanted != &result_ty {
                        // Checked view-to-view narrowing (Profile 0.14,
                        // WEB-P-006): a derived view re-satisfies a narrower
                        // same-element expectation through an explicit
                        // runtime span check instead of refusing.
                        if self.profile_0_14() {
                            if let Some(narrowed) = self.narrow_view_for_expected(
                                &ResolvedBinding::plain(id.clone(), result_ty.clone()),
                                wanted,
                            ) {
                                return Some(narrowed);
                            }
                        }
                        diagnostics.push(elaboration_diagnostic(
                            "MNE188",
                            format!(
                                "derived view has type {} which does not satisfy the required type",
                                result_ty.semantic_name()
                            ),
                            *span,
                        ));
                    }
                }
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
                    self.admitted_sequence_ceiling(),
                );
                let is_float =
                    |ty: &BodyType| matches!(ty, BodyType::Float(float) if float.is_supported());
                let convertible_source = |ty: &BodyType| {
                    matches!(ty, BodyType::Byte)
                        || matches!(ty, BodyType::Integer(integer) if matches!(integer.bits, 1..=64))
                        || matches!(ty, BodyType::Named(name) if name == "bool")
                        || is_float(ty)
                };
                let convertible_target = |ty: &BodyType| {
                    matches!(ty, BodyType::Byte)
                        || matches!(ty, BodyType::Integer(integer) if matches!(integer.bits, 1..=64))
                        || is_float(ty)
                };
                if !convertible_source(&subject.ty) || !convertible_target(&to) {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE189",
                        "explicit conversions require scalar integer, byte, or binary64 float types",
                        *span,
                    ));
                    return None;
                }
                // Conversions touching floats produce float values: int/bool
                // to float rounds per IEEE-754, float to int truncates toward
                // zero and traps on non-finite or out-of-range inputs.
                if (is_float(&subject.ty) || is_float(&to)) && !self.profile_float() {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE247",
                        "float values require source profile 0.12 or later",
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
            AstExpr::Not { value, .. } => {
                // Logical negation (Profile 0.13, CP-0004): `!bool -> bool`
                // through the dedicated total `BooleanNot` operation — never
                // a frontend rewrite — so semantic identities, obligations,
                // and every backend observe the same operation. The parser
                // only produces `Not` in Profile 0.13+; this gate is
                // defense in depth for a smuggled AST.
                if !self.profile_0_13() {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE149",
                        "logical negation requires source profile 0.13 or later",
                        expr.span(),
                    ));
                    return None;
                }
                let bool_type = BodyType::Named("bool".to_owned());
                // No expectation threading: like `&&`/`||`, the operand is
                // elaborated in its own type and checked here, so a
                // mistyped operand reports the single precise MNE181.
                let operand = self.elaborate_expr(value, None, env, diagnostics)?;
                if operand.ty != bool_type {
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
                    kind: BodyOperationKind::BooleanNot,
                    operands: vec![operand.id],
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
                Some(ResolvedBinding::plain(id, bool_type))
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
                // Thread the result expectation into literal operands for
                // total operators (arithmetic and bitwise alike, including
                // the wrapping/saturating/division spellings, which share
                // the same literal-adaptation rule).
                let threads_expected = matches!(
                    op,
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
                        | AstBinaryOp::MulSat
                        | AstBinaryOp::BitwiseAnd
                        | AstBinaryOp::BitwiseOr
                        | AstBinaryOp::BitwiseXor
                );
                let operand_expected = threads_expected.then_some(expected).flatten();
                // Symmetric literal adaptation (WEB-P-005): when the left
                // operand is a bare integer literal, elaborate the right
                // side first so the literal adapts to the right side's
                // concrete type exactly as a right-side literal adapts to
                // the left's. `1000 +% x` then means what `x +% 1000`
                // means. Comparisons keep their strict order (mixed-width
                // refusal is pinned behavior, not an adaptation gap).
                let (left_value, right_value) =
                    if threads_expected && matches!(left.as_ref(), AstExpr::Integer { .. }) {
                        let right_value =
                            self.elaborate_expr(right, operand_expected, env, diagnostics)?;
                        let left_value =
                            self.elaborate_expr(left, Some(&right_value.ty), env, diagnostics)?;
                        (left_value, right_value)
                    } else {
                        let left_value =
                            self.elaborate_expr(left, operand_expected, env, diagnostics)?;
                        let right_value =
                            self.elaborate_expr(right, Some(&left_value.ty), env, diagnostics)?;
                        (left_value, right_value)
                    };
                if left_value.ty != right_value.ty {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE119",
                        format!(
                            "binary operands must have the same type (left: {}, right: {}); convert explicitly with `as` (e.g. `left as {}`)",
                            left_value.ty.semantic_name(),
                            right_value.ty.semantic_name(),
                            right_value.ty.semantic_name()
                        ),
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
                // Boolean equality (Profile 0.13, CP-0004): `bool == bool
                // -> bool` and `bool != bool -> bool` through the dedicated
                // total `BooleanCompare` operation. Older profiles skip this
                // block and keep the historical MNE121 refusal below.
                // Mixed-type operands never reach here: the MNE119
                // same-type check above rejects them first. Ordering
                // comparisons on bools fall through to the integer gate
                // below (MNE121).
                if self.profile_0_13() && matches!(op, AstBinaryOp::Eq | AstBinaryOp::Ne) {
                    let bool_type = BodyType::Named("bool".to_owned());
                    if left_value.ty == bool_type && right_value.ty == bool_type {
                        let id = self.new_value("b");
                        self.blocks[self.current].operations.push(BodyOperation {
                            id: id.clone(),
                            kind: BodyOperationKind::BooleanCompare {
                                predicate: match op {
                                    AstBinaryOp::Eq => "eq".to_owned(),
                                    _ => "ne".to_owned(),
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
                // Integer bitwise operators: total exact-width operations under
                // wrapping intent, so no overflow obligation survives. Both
                // operands already share one integer type (MNE119 above);
                // signedness is irrelevant to the bit pattern, and the
                // result keeps the operand width. Non-integer operands fall
                // through to the comparison catch-all and its diagnostics.
                if matches!(
                    op,
                    AstBinaryOp::BitwiseAnd | AstBinaryOp::BitwiseOr | AstBinaryOp::BitwiseXor
                ) {
                    if let BodyType::Integer(operand_type) = left_value.ty {
                        let id = self.new_value("b");
                        let kind = BodyOperationKind::Integer {
                            operator: match op {
                                AstBinaryOp::BitwiseAnd => "and".to_owned(),
                                AstBinaryOp::BitwiseOr => "or".to_owned(),
                                _ => "xor".to_owned(),
                            },
                            operand_type,
                            intent: ArithmeticIntent::Wrapping,
                        };
                        let result_ty = BodyType::Integer(operand_type);
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
                }
                // Binary64 operators (Profile 0.12): `+ - * /` under the
                // non-finite trap rule, and the six comparisons. Both
                // operands already share the float type (MNE119 above);
                // mixed int/float arithmetic stays refused, and every other
                // operator (bitwise, shifts, `%`, wrapping/saturating
                // intents) is refused explicitly rather than falling into
                // the integer diagnostics below.
                if let BodyType::Float(float) = left_value.ty {
                    if !self.profile_float() {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE247",
                            "float values require source profile 0.12 or later",
                            expr.span(),
                        ));
                        return None;
                    }
                    if !float.is_supported() {
                        diagnostics.push(elaboration_diagnostic(
                            "MNE249",
                            "only binary64 floats are supported",
                            expr.span(),
                        ));
                        return None;
                    }
                    let is_compare = matches!(
                        op,
                        AstBinaryOp::Eq
                            | AstBinaryOp::Ne
                            | AstBinaryOp::Lt
                            | AstBinaryOp::Le
                            | AstBinaryOp::Gt
                            | AstBinaryOp::Ge
                    );
                    let operator = match op {
                        AstBinaryOp::Add => "add",
                        AstBinaryOp::Sub => "sub",
                        AstBinaryOp::Mul => "mul",
                        AstBinaryOp::Div => "div",
                        _ if is_compare => "",
                        _ => {
                            diagnostics.push(elaboration_diagnostic(
                                "MNE248",
                                "only `+ - * /` and comparisons are defined on floats",
                                expr.span(),
                            ));
                            return None;
                        }
                    };
                    let id = self.new_value("fl");
                    let (kind, result_ty) = if is_compare {
                        (
                            BodyOperationKind::FloatCompare {
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
                            BodyOperationKind::Float {
                                operator: operator.to_owned(),
                            },
                            BodyType::Float(float),
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
        let mut value = value;
        if value.ty != self.output_type {
            let output_type = self.output_type.clone();
            if let Some(borrowed) = self.borrow_view_for_expected(&value, &output_type) {
                value = borrowed;
            } else {
                diagnostics.push(elaboration_diagnostic(
                    "MNE103",
                    "returned value type does not match the declared output type",
                    span,
                ));
            }
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
                    self.admitted_sequence_ceiling(),
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

    /// Hygienic compiler temporary identity, by construction.
    ///
    /// Source identifiers start with `[A-Za-z_]` and continue with
    /// `[A-Za-z0-9_]` (`mncs-syntax`), so no source binding can spell a
    /// `$`-prefixed identity. Every elaborator temporary therefore lives in
    /// a namespace user code cannot name, and short prefixes like `c`/`b`
    /// can never alias user bindings such as `c0`/`b3` (MNB011). The counter
    /// keeps emission deterministic.
    /// Hygienic compiler temporary identity, by construction.
    ///
    /// Source identifiers start with `[A-Za-z_]` and continue with
    /// `[A-Za-z0-9_]` (`mncs-syntax`), so no source binding can spell a
    /// `$`-prefixed identity. Every elaborator temporary therefore lives in
    /// a namespace user code cannot name, and short prefixes like `c`/`b`
    /// can never alias user bindings such as `c0`/`b3` (MNB011). The counter
    /// keeps emission deterministic.
    fn new_value(&mut self, prefix: &str) -> String {
        let id = format!("$mncs${prefix}${}", self.next_value);
        self.next_value += 1;
        id
    }

    /// Exact-to-bounded-view borrow at an expectation site: when `expected`
    /// is `[E; up_to M]` and the elaborated value is `[E; N]` with `N <= M`,
    /// synthesize the full-range slice and return the view-typed binding.
    /// Returns `None` when the rule does not apply (the caller emits its
    /// own diagnostic). The borrow is explicit in the body and lowers
    /// through the proven view machinery: no copy is materialized, the
    /// bound is preserved from the static length, and element identity is
    /// untouched. `N > M`, element mismatch, and non-sequence shapes all
    /// correctly refuse.
    fn borrow_view_for_expected(
        &mut self,
        binding: &ResolvedBinding,
        expected: &BodyType,
    ) -> Option<ResolvedBinding> {
        if let Some((length, capacity)) = exact_view_borrow_dimensions(&binding.ty, expected) {
            return Some(self.borrow_exact_as_view(binding, length, capacity));
        }
        // Checked view-to-view narrowing (Profile 0.14, WEB-P-006): a view
        // already in hand re-satisfies a narrower same-element expectation
        // through an explicit runtime span check, at every site the borrow
        // backstop already serves.
        if self.profile_0_14() && view_narrow_dimensions(&binding.ty, expected).is_some() {
            return self.narrow_view_for_expected(binding, expected);
        }
        None
    }

    /// Deterministic generic-argument inference (ENG-PRESSURE-0019): solve
    /// callee generic parameters from directly-constrained value arguments
    /// — e.g. `grow_fill(base, 7)` with `base: [i64; 8]` pins `W = 8` —
    /// with no search and no Hindley-Milner machinery. Constraints flow
    /// through sequence structure (`[i64; W]` against `[i64; 8]`) and
    /// direct generic positions (`T` against `i64`); nominal wrappers
    /// (records, finite types) are opaque, views (`up_to`) never pin
    /// lengths, and every parameter must end with exactly one answer.
    /// Caller-parameter forwarding (`N := M`, `T := U`) infers the same
    /// `ValueParam`/generic `Type` the explicit spelling produces.
    /// Anything else — unconstrained parameters, conflicting constraints,
    /// arity mismatch — returns the failure so the caller keeps the
    /// explicit-argument diagnostic; explicit `<...>` always remains.
    fn infer_generic_args(
        &mut self,
        signature: &FunctionSignature,
        arguments: &[AstExpr],
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Result<Vec<mncs_model::GenericArg>, GenericInferenceFailure> {
        use GenericInferenceFailure::{Ambiguous, ArgError, Arity};
        if arguments.len() != signature.inputs.len() {
            return Err(Arity);
        }
        // Speculative elaboration: argument operations, resolutions, and
        // diagnostics are rolled back below; the real elaboration pass
        // re-derives them against the solved signature.
        let ops_len = self.blocks[self.current].operations.len();
        let resolutions_len = self.resolutions.len();
        let diagnostics_len = diagnostics.len();
        let mut actuals = Vec::new();
        for source_argument in arguments {
            let Some(binding) = self.elaborate_expr(source_argument, None, env, diagnostics) else {
                self.blocks[self.current].operations.truncate(ops_len);
                self.resolutions.truncate(resolutions_len);
                return Err(ArgError);
            };
            actuals.push(binding.ty);
        }
        let mut nats: BTreeMap<String, Vec<InferredNat>> = BTreeMap::new();
        let mut types: BTreeMap<String, Vec<InferredTy>> = BTreeMap::new();
        for (declared, actual) in signature.inputs.iter().zip(&actuals) {
            collect_inference_constraints(declared, actual, &mut nats, &mut types);
        }
        let mut args_out = Vec::new();
        let mut missing = Vec::new();
        let mut conflicting = Vec::new();
        for param in &signature.generic_params {
            match param.kind {
                mncs_model::GenericParamKind::Nat => {
                    match nats.get(&param.name).map(|options| {
                        let mut distinct = options.clone();
                        distinct.sort();
                        distinct.dedup();
                        distinct
                    }) {
                        Some(options) if options.len() == 1 => args_out.push(match &options[0] {
                            InferredNat::Value(value) => {
                                mncs_model::GenericArg::Value { value: *value }
                            }
                            InferredNat::Forward(name) => {
                                mncs_model::GenericArg::ValueParam { name: name.clone() }
                            }
                        }),
                        Some(_) => conflicting.push(param.name.clone()),
                        None => missing.push(param.name.clone()),
                    }
                }
                mncs_model::GenericParamKind::Type => {
                    match types.get(&param.name).map(|options| {
                        let mut distinct = options.clone();
                        distinct.dedup();
                        distinct
                    }) {
                        Some(options) if options.len() == 1 => args_out.push(match &options[0] {
                            InferredTy::Concrete(ty) => {
                                mncs_model::GenericArg::Type { ty: ty.clone() }
                            }
                            InferredTy::Forward(name) => mncs_model::GenericArg::Type {
                                ty: BodyType::GenericParam { name: name.clone() },
                            },
                        }),
                        Some(_) => conflicting.push(param.name.clone()),
                        None => missing.push(param.name.clone()),
                    }
                }
            }
        }
        // Roll back the speculative pass in every outcome; success
        // re-elaborates against solved expectations, failure emits no
        // artifact, and diagnostics rollback is safe because a successful
        // speculative pass carries no errors for the real pass to lose.
        self.blocks[self.current].operations.truncate(ops_len);
        self.resolutions.truncate(resolutions_len);
        if missing.is_empty() && conflicting.is_empty() {
            diagnostics.truncate(diagnostics_len);
            Ok(args_out)
        } else {
            Err(Ambiguous {
                missing,
                conflicting,
            })
        }
    }

    /// Elaborate `checked_index(sequence, index)` (Profile 0.14, WEB-P-009):
    /// the retained bounds check behind `CheckedBound` discharge. The
    /// candidate must be `u64` and the sequence bounded; a literal
    /// candidate at or beyond an exact declared bound fails closed here.
    /// The result carries the candidate unchanged, and the value id is
    /// recorded against the sequence value id so a later projection
    /// through exactly this value discharges with `CheckedBound` evidence.
    fn elaborate_checked_index(
        &mut self,
        sequence: &AstExpr,
        index: &AstExpr,
        span: SourceSpan,
        expected: Option<&BodyType>,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        if !self.profile_0_14() {
            diagnostics.push(elaboration_diagnostic(
                "MNE272",
                "checked index requires source profile 0.14 or later",
                span,
            ));
            return None;
        }
        let subject = self.elaborate_expr(sequence, None, env, diagnostics)?;
        let counter_type = BodyType::Integer(IntegerType {
            bits: 64,
            signed: false,
        });
        let candidate = self.elaborate_expr(index, Some(&counter_type), env, diagnostics)?;
        if candidate.ty != counter_type {
            diagnostics.push(elaboration_diagnostic(
                "MNE274",
                "checked index candidate must have u64 type",
                index.span(),
            ));
            return None;
        }
        // A literal candidate against an exact bound is decided here, like
        // an index projection: provably outside fails closed instead of
        // deferring a guaranteed trap. In-range literals still emit the
        // check so the recorded fact (not literal syntax) discharges uses
        // through the bound name.
        if let (
            BodyType::Sequence {
                bound: mncs_model::SequenceBound::Exact(length),
                ..
            },
            AstExpr::Integer { value, .. },
        ) = (&subject.ty, index)
        {
            if *value < 0 || *value >= i128::from(*length) {
                diagnostics.push(elaboration_diagnostic(
                    "MNE275",
                    format!(
                        "checked index {value} is outside the statically known domain 0..{length}"
                    ),
                    index.span(),
                ));
                return None;
            }
        }
        if expected.is_some_and(|expected| expected != &counter_type) {
            diagnostics.push(elaboration_diagnostic(
                "MNE276",
                "checked index does not have the required expression type",
                span,
            ));
        }
        let BodyType::Sequence { bound, .. } = subject.ty.clone() else {
            diagnostics.push(elaboration_diagnostic(
                "MNE273",
                "checked index requires a bounded-sequence value",
                sequence.span(),
            ));
            return None;
        };
        let id = self.new_value("chk");
        self.blocks[self.current].operations.push(BodyOperation {
            id: id.clone(),
            kind: BodyOperationKind::BoundCheck {
                bound: bound.clone(),
            },
            operands: vec![subject.id.clone(), candidate.id],
            results: vec![BodyValue {
                id: id.clone(),
                ty: counter_type.clone(),
            }],
            contracts: Vec::new(),
            assumptions: Vec::new(),
            machine_intent: None,
            lowering: None,
            portability: None,
        });
        self.checked_indices.insert(id.clone(), subject.id);
        Some(ResolvedBinding::plain(id, counter_type))
    }

    /// Elaborate `copy_span(dst, dst_at, src, src_at, len)` (Profile 0.14,
    /// WEB-P-003): a total functional bounded span copy. The destination
    /// must be an exact-bound sequence (views refuse, mirroring `replace`);
    /// the source may be exact or a view so staged and parsed spans copy
    /// without remarshal. All three positions are u64. Literal windows
    /// inside both static bounds establish `StaticExact` evidence; a
    /// provably out-of-range literal window fails closed here; anything
    /// else keeps an explicit runtime-checked failure obligation.
    #[allow(clippy::too_many_arguments)]
    fn elaborate_sequence_copy(
        &mut self,
        destination: &AstExpr,
        dst_at: &AstExpr,
        source: &AstExpr,
        src_at: &AstExpr,
        len: &AstExpr,
        span: SourceSpan,
        _expected: Option<&BodyType>,
        env: &mut BindingEnv,
        diagnostics: &mut Vec<SourceDiagnostic>,
    ) -> Option<ResolvedBinding> {
        if !self.profile_0_14() {
            diagnostics.push(elaboration_diagnostic(
                "MNE263",
                "span copy requires source profile 0.14 or later",
                span,
            ));
            return None;
        }
        let dst = self.elaborate_expr(destination, None, env, diagnostics)?;
        let BodyType::Sequence {
            element: element_type,
            bound: dst_bound,
        } = dst.ty.clone()
        else {
            diagnostics.push(elaboration_diagnostic(
                "MNE264",
                "span copy requires a bounded-sequence destination",
                destination.span(),
            ));
            return None;
        };
        if matches!(dst_bound, mncs_model::SequenceBound::UpTo(_)) {
            diagnostics.push(elaboration_diagnostic(
                "MNE265",
                "span copy requires an exact-bound destination this tranche; views refuse",
                span,
            ));
            return None;
        }
        let counter_type = BodyType::Integer(IntegerType {
            bits: 64,
            signed: false,
        });
        let dst_at_binding = self.elaborate_expr(dst_at, Some(&counter_type), env, diagnostics)?;
        if dst_at_binding.ty != counter_type {
            diagnostics.push(elaboration_diagnostic(
                "MNE266",
                "span copy destination offset must have u64 type",
                dst_at.span(),
            ));
            return None;
        }
        let src = self.elaborate_expr(source, None, env, diagnostics)?;
        let BodyType::Sequence {
            element: src_element,
            bound: src_bound,
        } = src.ty.clone()
        else {
            diagnostics.push(elaboration_diagnostic(
                "MNE267",
                "span copy requires a bounded-sequence source",
                source.span(),
            ));
            return None;
        };
        if *src_element != *element_type {
            diagnostics.push(elaboration_diagnostic(
                "MNE268",
                "span copy source element does not match the destination element type",
                source.span(),
            ));
            return None;
        }
        let src_at_binding = self.elaborate_expr(src_at, Some(&counter_type), env, diagnostics)?;
        if src_at_binding.ty != counter_type {
            diagnostics.push(elaboration_diagnostic(
                "MNE269",
                "span copy source offset must have u64 type",
                src_at.span(),
            ));
            return None;
        }
        let len_binding = self.elaborate_expr(len, Some(&counter_type), env, diagnostics)?;
        if len_binding.ty != counter_type {
            diagnostics.push(elaboration_diagnostic(
                "MNE270",
                "span copy length must have u64 type",
                len.span(),
            ));
            return None;
        }
        // Literal windows against static bounds are decided here: inside
        // both bounds is static evidence, provably outside either bound
        // fails closed rather than deferring a guaranteed runtime failure.
        // View sources have dynamic lengths, so a literal window over a
        // view source always stays runtime-checked.
        let literal_window = match (dst_at, src_at, len) {
            (
                AstExpr::Integer { value: dst_at, .. },
                AstExpr::Integer { value: src_at, .. },
                AstExpr::Integer { value: len, .. },
            ) => Some((*dst_at, *src_at, *len)),
            _ => None,
        };
        let evidence = match (literal_window, &dst_bound, &src_bound) {
            (
                Some((dst_at, src_at, len)),
                mncs_model::SequenceBound::Exact(dst_length),
                mncs_model::SequenceBound::Exact(src_length),
            ) => {
                if dst_at >= 0
                    && src_at >= 0
                    && len >= 0
                    && (dst_at as u128) + (len as u128) <= u128::from(*dst_length)
                    && (src_at as u128) + (len as u128) <= u128::from(*src_length)
                {
                    mncs_model::BoundsEvidence::StaticExact
                } else {
                    diagnostics.push(elaboration_diagnostic(
                        "MNE271",
                        "span copy window is outside the statically known bounds",
                        span,
                    ));
                    return None;
                }
            }
            // View sources have dynamic lengths, so any window over them —
            // literal or not — stays runtime-checked.
            _ => mncs_model::BoundsEvidence::RuntimeChecked {
                failure: FailureMode::Isolated,
            },
        };
        let result_type = BodyType::Sequence {
            element: element_type.clone(),
            bound: dst_bound.clone(),
        };
        let id = self.new_value("cpy");
        self.blocks[self.current].operations.push(BodyOperation {
            id: id.clone(),
            kind: BodyOperationKind::SequenceCopy {
                element_type: element_type.clone(),
                dst_bound: dst_bound.clone(),
                src_bound: src_bound.clone(),
                evidence: evidence.clone(),
            },
            operands: vec![
                dst.id,
                dst_at_binding.id,
                src.id,
                src_at_binding.id,
                len_binding.id,
            ],
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

    /// Borrow an exact sequence as a bounded view (`[E; N]` to `[E; up_to M]`
    /// with `N <= M`, same element type) by synthesizing the full-range
    /// slice. The borrow is explicit in the body and lowers through the
    /// proven view machinery: no copy is materialized, the bound is
    /// preserved from the static length, and element identity is untouched.
    /// Callers must have established `N <= M` and element equality already.
    fn borrow_exact_as_view(
        &mut self,
        argument: &ResolvedBinding,
        length: u32,
        capacity: u32,
    ) -> ResolvedBinding {
        let element = match &argument.ty {
            BodyType::Sequence { element, .. } => (**element).clone(),
            _ => BodyType::Named("invalid".to_owned()),
        };
        let counter = BodyType::Integer(IntegerType {
            bits: 64,
            signed: false,
        });
        let mut constant = |value: i128| {
            let id = self.new_value("c");
            self.blocks[self.current].operations.push(BodyOperation {
                id: id.clone(),
                kind: BodyOperationKind::Constant {
                    value,
                    ty: counter.clone(),
                },
                operands: Vec::new(),
                results: vec![BodyValue {
                    id: id.clone(),
                    ty: counter.clone(),
                }],
                contracts: Vec::new(),
                assumptions: Vec::new(),
                machine_intent: None,
                lowering: None,
                portability: None,
            });
            id
        };
        let start_id = constant(0);
        let end_id = constant(i128::from(length));
        let result_ty = BodyType::Sequence {
            element: Box::new(element),
            bound: mncs_model::SequenceBound::UpTo(capacity),
        };
        let id = self.new_value("view");
        self.blocks[self.current].operations.push(BodyOperation {
            id: id.clone(),
            kind: BodyOperationKind::ViewConstruct {
                source_bound: mncs_model::SequenceBound::Exact(length),
                view_bound: mncs_model::SequenceBound::UpTo(capacity),
            },
            operands: vec![argument.id.clone(), start_id, end_id],
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
        ResolvedBinding::plain(id, result_ty)
    }

    /// Narrow a view already in hand (`[E; up_to A]` to `[E; up_to B]`
    /// with `B < A`, same element type) by synthesizing the explicit
    /// runtime span check. Callers must have established the dimensions
    /// already; the check fails closed when the live span escapes the new
    /// capacity, and no copy is materialized.
    fn narrow_view_for_expected(
        &mut self,
        argument: &ResolvedBinding,
        expected: &BodyType,
    ) -> Option<ResolvedBinding> {
        let (source_cap, new_cap) = view_narrow_dimensions(&argument.ty, expected)?;
        let element = match &argument.ty {
            BodyType::Sequence { element, .. } => (**element).clone(),
            _ => BodyType::Named("invalid".to_owned()),
        };
        let result_ty = BodyType::Sequence {
            element: Box::new(element),
            bound: mncs_model::SequenceBound::UpTo(new_cap),
        };
        let id = self.new_value("narrow");
        self.blocks[self.current].operations.push(BodyOperation {
            id: id.clone(),
            kind: BodyOperationKind::ViewNarrow {
                source_cap,
                new_cap,
            },
            operands: vec![argument.id.clone()],
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

/// Exact-to-bounded-view borrow rule (`docs/source-profile-0.7.md`): an
/// `[E; N]` value satisfies an `[E; up_to M]` expectation exactly when
/// `N <= M`. Returns the static `(length, capacity)` the synthesized
/// full-range slice must carry. Everything else correctly refuses.
fn exact_view_borrow_dimensions(actual: &BodyType, expected: &BodyType) -> Option<(u32, u32)> {
    if let (
        BodyType::Sequence {
            element: actual_element,
            bound: mncs_model::SequenceBound::Exact(length),
        },
        BodyType::Sequence {
            element: expected_element,
            bound: mncs_model::SequenceBound::UpTo(capacity),
        },
    ) = (actual, expected)
    {
        if actual_element == expected_element && length <= capacity {
            return Some((*length, *capacity));
        }
    }
    None
}

/// View-to-view narrowing rule (Profile 0.14): an `[E; up_to A]` value
/// satisfies an `[E; up_to B]` expectation exactly when `B < A` with the
/// same element type. Returns the `(source capacity, new capacity)` the
/// synthesized span check must carry. Equal capacities need no check (the
/// types already agree), wider targets refuse, and everything else
/// correctly refuses.
fn view_narrow_dimensions(actual: &BodyType, expected: &BodyType) -> Option<(u32, u32)> {
    if let (
        BodyType::Sequence {
            element: actual_element,
            bound: mncs_model::SequenceBound::UpTo(source_cap),
        },
        BodyType::Sequence {
            element: expected_element,
            bound: mncs_model::SequenceBound::UpTo(new_cap),
        },
    ) = (actual, expected)
    {
        if actual_element == expected_element && new_cap < source_cap {
            return Some((*source_cap, *new_cap));
        }
    }
    None
}

/// Admitted sequence/view length ceiling for source spelling resolution
/// under `ast`'s profile (RFC 0036): 64 through Profile 0.12, 1024 in
/// Profile 0.13. Unknown profiles fail closed to zero.
fn admitted_sequence_ceiling(ast: &AbstractSyntaxTree) -> u32 {
    mncs_syntax::max_sequence_bound_for(&ast.language_version.text).unwrap_or(0)
}

fn profile_type(
    name: &str,
    span: SourceSpan,
    finite_types: &BTreeMap<String, FiniteType>,
    record_types: &BTreeMap<String, RecordType>,
    diagnostics: &mut Vec<SourceDiagnostic>,
    sequence_ceiling: u32,
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
    if let Some(sequence) =
        profile_sequence_type(name, finite_types, record_types, sequence_ceiling)
    {
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
    sequence_ceiling: u32,
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
            sequence_ceiling,
        ) {
            return seq;
        }
    }
    // Defer to non-generic resolver (finite/record/scalar/sequence without generics)
    profile_type(
        name,
        span,
        finite_types,
        record_types,
        diagnostics,
        sequence_ceiling,
    )
}

/// Why generic-argument inference declined a call (ENG-PRESSURE-0019).
/// The caller maps every variant back to the explicit-argument diagnostic
/// so inference never invents a new failure mode.
enum GenericInferenceFailure {
    /// Arity differs; the standard arity diagnostic follows.
    Arity,
    /// An argument failed elaboration on its own; its diagnostics stand.
    ArgError,
    /// These parameters admit no unique answer: `missing` drew no
    /// constraint from any argument, `conflicting` drew two or more
    /// distinct answers.
    Ambiguous {
        missing: Vec<String>,
        conflicting: Vec<String>,
    },
}

/// One Nat constraint from one argument position: an exact length or a
/// caller-parameter forward.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum InferredNat {
    Value(u32),
    Forward(String),
}

/// One type constraint from one argument position: a concrete type or a
/// caller-parameter forward.
#[derive(Clone, PartialEq, Eq)]
enum InferredTy {
    Concrete(BodyType),
    Forward(String),
}

/// Collect Nat/type constraints by walking declared and actual types in
/// lockstep. Sequences recurse structurally; a direct generic position
/// pins its parameter; everything else (nominals, views against views,
/// mismatched shapes) constrains nothing and lets ambiguity refuse.
fn collect_inference_constraints(
    declared: &BodyType,
    actual: &BodyType,
    nats: &mut BTreeMap<String, Vec<InferredNat>>,
    types: &mut BTreeMap<String, Vec<InferredTy>>,
) {
    match (declared, actual) {
        (BodyType::GenericParam { name }, BodyType::GenericParam { name: caller }) => {
            types
                .entry(name.clone())
                .or_default()
                .push(InferredTy::Forward(caller.clone()));
        }
        (BodyType::GenericParam { name }, actual) => {
            types
                .entry(name.clone())
                .or_default()
                .push(InferredTy::Concrete(actual.clone()));
        }
        (
            BodyType::Sequence {
                element: declared_element,
                bound: declared_bound,
            },
            BodyType::Sequence {
                element: actual_element,
                bound: actual_bound,
            },
        ) => {
            match (declared_bound, actual_bound) {
                (
                    mncs_model::SequenceBound::Param(name),
                    mncs_model::SequenceBound::Exact(value),
                ) => {
                    nats.entry(name.clone())
                        .or_default()
                        .push(InferredNat::Value(*value));
                }
                (
                    mncs_model::SequenceBound::Param(name),
                    mncs_model::SequenceBound::Param(caller),
                ) => {
                    nats.entry(name.clone())
                        .or_default()
                        .push(InferredNat::Forward(caller.clone()));
                }
                _ => {}
            }
            collect_inference_constraints(declared_element, actual_element, nats, types);
        }
        _ => {}
    }
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
        BodyType::Float(f) => BodyType::Float(f),
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
            // Descend the source spelling alongside the type: passing the
            // outer spelling down re-wrapped one nesting level per import
            // (ENG-PRESSURE-0011: `[[i64; 2]; 2]` linked as three levels).
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
        _ => canonical_value_type(source, ty),
    }
}

/// Resolve a canonical bounded-sequence spelling `[E; N]` / `[E; up_to M]`
/// whose element may be a scalar, byte, nested sequence, or a declared
/// nominal type of this module.
/// Admitted sequence/view length ceiling for source spelling resolution.
/// Callers pass the active profile's registry ceiling; import-path
/// re-resolution of already-validated artifacts passes the absolute model
/// ceiling (admission was enforced at origin elaboration).
fn profile_sequence_type(
    name: &str,
    finite_types: &BTreeMap<String, FiniteType>,
    record_types: &BTreeMap<String, RecordType>,
    sequence_ceiling: u32,
) -> Option<BodyType> {
    let inner = name.strip_prefix('[')?.strip_suffix(']')?;
    let separator = inner.rfind(';')?;
    let (element_text, bound_text) = (inner[..separator].trim(), inner[separator + 1..].trim());
    let bound = if let Some(capacity) = bound_text.strip_prefix("up_to") {
        let capacity = capacity.trim().parse::<u32>().ok()?;
        if capacity > sequence_ceiling {
            return None;
        }
        mncs_model::SequenceBound::UpTo(capacity)
    } else {
        let length = bound_text.parse::<u32>().ok()?;
        if length > sequence_ceiling {
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
        None => profile_sequence_type(element_text, finite_types, record_types, sequence_ceiling)?,
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
    sequence_ceiling: u32,
) -> Option<BodyType> {
    let inner = name.strip_prefix('[')?.strip_suffix(']')?;
    let separator = inner.rfind(';')?;
    let (element_text, bound_text) = (inner[..separator].trim(), inner[separator + 1..].trim());
    let bound = if let Some(capacity) = bound_text.strip_prefix("up_to") {
        let cap = capacity.trim();
        if let Ok(v) = cap.parse::<u32>() {
            if v > sequence_ceiling {
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
        if v > sequence_ceiling {
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
        sequence_ceiling,
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
        )
        // `f64` resolves as a type name on every profile so declarations
        // keep one spelling; producing float values (literals, operators,
        // casts, comparisons) is gated on Profile 0.12 at each use site.
        || matches!(&ty, BodyType::Float(float) if float.is_supported());
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
