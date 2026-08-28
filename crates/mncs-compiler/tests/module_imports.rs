use std::collections::BTreeMap;

use mncs_compiler::{
    elaborate_program_with_resolver, elaborate_program_with_resolver_and_modules, ModuleResolver,
    NullResolver, ReferenceCompiler, ResolvedNameKind,
};
use mncs_syntax::{parse, SourceArtifactKind, SourceEnvelope};

#[derive(Default)]
struct MapResolver {
    modules: BTreeMap<String, String>,
}

impl MapResolver {
    fn with(mut self, name: &str, text: &str) -> Self {
        self.modules.insert(name.to_owned(), text.to_owned());
        self
    }

    fn envelope(&self, name: &str, text: String) -> SourceEnvelope {
        SourceEnvelope::inline(SourceArtifactKind::Program, name.to_owned(), text)
    }
}

impl ModuleResolver for MapResolver {
    fn resolve(&self, module: &str) -> Option<SourceEnvelope> {
        self.modules
            .get(module)
            .cloned()
            .map(|text| self.envelope(module, text))
    }
}

fn elaborate(resolver: &MapResolver, text: &str) -> Result<mncs_model::Program, Vec<String>> {
    let envelope = resolver.envelope("root", text.to_owned());
    let parsed = parse(&envelope);
    let ast = parsed.ast.expect("fixture parses");
    match elaborate_program_with_resolver(&ast, resolver).0 {
        Ok(program) => Ok(program),
        Err(errors) => Err(errors.iter().map(|d| d.code.clone()).collect()),
    }
}

fn evidence_module() -> String {
    r#"
mncs 0.6;
module lib.evidence;
enum Verdict { PASS, FAIL, UNKNOWN }
record GateInput { present: bool, observed: i64, threshold: i64 }
fn evaluate(input: GateInput) -> (verdict: Verdict)
{
    let comparable: bool = input.present;
    if comparable {
        let holds: bool = input.observed >= input.threshold;
        if holds {
            return Verdict.PASS;
        }
        return Verdict.FAIL;
    }
    return Verdict.UNKNOWN;
}
"#
    .to_owned()
}

#[test]
fn imported_types_and_functions_participate_in_calling_module() {
    let resolver = MapResolver::default().with("lib.evidence", &evidence_module());
    let program = elaborate(
        &resolver,
        r#"
mncs 0.6;
module app.study;
use lib.evidence;
enum Outcome { ACCEPT, DEFER }
record Panel { present: bool, observed: i64, threshold: i64 }
fn decide(input: GateInput, fallback: GateInput) -> (outcome: Outcome)
{
    let left: Verdict = evaluate(input);
    let right: Verdict = evaluate(fallback);
    return match left {
        PASS => match right {
            PASS => Outcome.ACCEPT,
            FAIL => Outcome.DEFER,
            UNKNOWN => Outcome.DEFER,
        },
        FAIL => Outcome.DEFER,
        UNKNOWN => Outcome.DEFER,
    };
}
"#,
    )
    .expect("linked elaboration succeeds");
    assert_eq!(program.functions.len(), 2);
    assert_eq!(program.finite_types.len(), 2);
    assert_eq!(program.record_types.len(), 2);
    // Dependency identity is recorded and anchored to the declaring module.
    assert_eq!(program.dependencies.len(), 1);
    assert_eq!(program.dependencies[0].0, "mncs:0.2:module:lib.evidence");
}

#[test]
fn authority_closure_applies_across_module_boundary() {
    let evidence = evidence_module().replace(
        "fn evaluate(input: GateInput) -> (verdict: Verdict)",
        "fn evaluate(input: GateInput) -> (verdict: Verdict)\n    capability hard_gate_authority\n    effect derive_verdict authorized_by hard_gate_authority",
    );
    let resolver = MapResolver::default().with("lib.evidence", &evidence);
    let errors = elaborate(
        &resolver,
        r#"
mncs 0.6;
module app.rogue;
use lib.evidence;
fn rogue(first: GateInput) -> (result: Verdict)
{
    return evaluate(first);
}
"#,
    )
    .unwrap_err();
    assert!(errors.contains(&"MNE134".to_owned()), "got {errors:?}");

    // The same call succeeds once the caller re-declares the authority.
    let program = elaborate(
        &resolver,
        r#"
mncs 0.6;
module app.authorized;
use lib.evidence;
fn authorized(first: GateInput) -> (result: Verdict)
    capability hard_gate_authority
    effect derive_verdict authorized_by hard_gate_authority
{
    return evaluate(first);
}
"#,
    )
    .expect("authority-carrying caller is accepted");
    assert_eq!(program.functions.len(), 2);
}

#[test]
fn missing_imported_module_is_a_diagnostic() {
    let resolver = MapResolver::default();
    let errors = elaborate(
        &resolver,
        "mncs 0.6;\nmodule lonely;\nuse nowhere;\nfn f(a: i64) -> (r: i64) { return a; }\n",
    )
    .unwrap_err();
    assert!(errors.contains(&"MNE173".to_owned()));
}

#[test]
fn import_cycles_are_rejected() {
    let resolver = MapResolver::default()
        .with(
            "a.first",
            "mncs 0.6;\nmodule a.first;\nuse b.second;\nfn f(a: i64) -> (r: i64) { return g(a); }\n",
        )
        .with(
            "b.second",
            "mncs 0.6;\nmodule b.second;\nuse a.first;\nfn g(a: i64) -> (r: i64) { return f(a); }\n",
        );
    let envelope = resolver.envelope(
        "root",
        "mncs 0.6;\nmodule root;\nuse a.first;\nfn h(a: i64) -> (r: i64) { return f(a); }\n"
            .to_owned(),
    );
    let parsed = parse(&envelope);
    let (outcome, _) = elaborate_program_with_resolver(parsed.ast.as_ref().unwrap(), &resolver);
    let errors = outcome.unwrap_err();
    assert!(
        errors.iter().any(|d| d.code == "MNE171"),
        "expected cycle diagnostic, got {:?}",
        errors.iter().map(|d| d.code.clone()).collect::<Vec<_>>()
    );
}

#[test]
fn imported_type_name_collision_fails_closed() {
    let resolver = MapResolver::default().with("lib.evidence", &evidence_module());
    let errors = elaborate(
        &resolver,
        r#"
mncs 0.6;
module app.clash;
use lib.evidence;
enum Verdict { YES, NO }
fn f(v: Verdict) -> (r: Verdict) { return v; }
"#,
    )
    .unwrap_err();
    assert!(errors.contains(&"MNE174".to_owned()), "got {errors:?}");
}

#[test]
fn imported_function_name_collision_fails_closed() {
    let resolver = MapResolver::default().with("lib.evidence", &evidence_module());
    let errors = elaborate(
        &resolver,
        r#"
mncs 0.6;
module app.clash;
use lib.evidence;
enum Local { A, B }
fn evaluate(v: Local) -> (r: Local) { return v; }
"#,
    )
    .unwrap_err();
    assert!(errors.contains(&"MNE176".to_owned()), "got {errors:?}");
}

#[test]
fn imported_name_resolutions_retain_the_declaring_source_span() {
    let resolver = MapResolver::default().with(
        "lib.evidence",
        "mncs 0.6;\nmodule lib.evidence;\nfn demote(value: i64) -> (result: i64) { return value; }\n",
    );
    let source = "mncs 0.6;\nmodule app.study;\nuse lib.evidence;\nfn soften(value: i64) -> (result: i64) { return demote(value); }\n";
    let root = resolver.envelope("root", source.to_owned());
    let dependency = resolver
        .modules
        .get("lib.evidence")
        .expect("dependency text");
    let dependency_ast = parse(&resolver.envelope("lib.evidence", dependency.clone()))
        .ast
        .expect("dependency parses");
    let declaration = dependency_ast.functions[0].name.span;
    let front_end = ReferenceCompiler::default().front_end_with_resolver(root, &resolver);
    let demote_offset = source.find("demote").expect("call site");
    let resolution = front_end
        .name_resolutions
        .at_offset(demote_offset)
        .next()
        .expect("imported call is resolved");
    assert_eq!(resolution.kind, ResolvedNameKind::Function);
    assert_eq!(resolution.declaration, declaration);
    assert_eq!(resolution.occurrence.start, demote_offset);
}

#[test]
fn imported_execution_and_lowering_keep_the_declaring_namespace() {
    let resolver = MapResolver::default().with(
        "lib.evidence",
        "mncs 0.6;\nmodule lib.evidence;\nfn demote(value: i64) -> (result: i64) { return value; }\n",
    );
    let source = "mncs 0.6;\nmodule app.study;\nuse lib.evidence;\nfn soften(value: i64) -> (result: i64) { return demote(value); }\n";
    let root = resolver.envelope("root", source.to_owned());
    let front_end = ReferenceCompiler::default().front_end_with_resolver(root, &resolver);
    assert!(front_end.is_valid(), "{:#?}", front_end.diagnostics);
    assert_eq!(front_end.module_resolutions.len(), 1);
    let resolution = &front_end.module_resolutions[0];
    assert_eq!(resolution.requested_module, "lib.evidence");
    assert_eq!(resolution.declared_module, "lib.evidence");
    assert_eq!(
        resolution.module_identity,
        mncs_model::module_id("lib.evidence")
    );
    assert_eq!(resolution.source_logical_name, "lib.evidence");
    assert!(resolution.semantic_fingerprint.is_some());

    let program = front_end.program.expect("linked program");
    let expected = mncs_model::function_id("lib.evidence", "demote");
    let imported = program
        .functions
        .iter()
        .find(|function| function.name == "demote")
        .expect("imported function");
    assert_eq!(imported.home_module.as_deref(), Some("lib.evidence"));
    assert!(program.semantic_identities().record(&expected).is_some());
    assert!(program
        .semantic_graph()
        .expect("semantic graph")
        .nodes
        .iter()
        .any(|node| node.identity == expected));
    assert!(program
        .lower_to_ir()
        .expect("HIR")
        .functions
        .iter()
        .any(|function| function.semantic_identity == expected));
    assert!(program
        .lower_to_ssa()
        .expect("SSA")
        .functions
        .iter()
        .any(|function| function.semantic_identity == expected));

    let request = mncs_model::ExecutionRequest {
        schema_version: mncs_model::EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
        target: mncs_model::ExecutionTarget {
            module: "app.study".to_owned(),
            function: "soften".to_owned(),
        },
        arguments: vec![mncs_model::ExecutionValue::Integer {
            value: 7,
            ty: mncs_model::IntegerType {
                bits: 64,
                signed: true,
            },
        }],
        step_budget: 64,
        policy: mncs_model::ExecutionPolicy::default(),
    };
    let body = mncs_model::execute_with_policy(&program, &request);
    assert_eq!(body.status, mncs_model::ExecutionStatus::Returned);
    assert_eq!(body.returned[0], request.arguments[0]);
    assert_eq!(
        body.function_identity,
        Some(mncs_model::function_id("app.study", "soften"))
    );
    assert!(body
        .trace
        .iter()
        .any(|entry| entry.block.0.contains(":block:lib.evidence::demote")));
    let ssa = mncs_model::execute_ssa(&program, &request);
    assert_eq!(ssa.status, mncs_model::ExecutionStatus::Returned);
    assert_eq!(ssa.returned, body.returned);
    assert_eq!(
        ssa.semantic_function_identity,
        Some(mncs_model::function_id("app.study", "soften"))
    );
}

#[test]
fn resolver_declared_module_mismatch_fails_closed() {
    let resolver = MapResolver::default().with(
        "lib.requested",
        "mncs 0.6;\nmodule lib.actual;\nfn value(input: i64) -> (result: i64) { return input; }\n",
    );
    let envelope = resolver.envelope(
        "root",
        "mncs 0.6;\nmodule app.root;\nuse lib.requested;\nfn f(input: i64) -> (result: i64) { return value(input); }\n"
            .to_owned(),
    );
    let ast = parse(&envelope).ast.expect("root parses");
    let (result, _, resolutions) = elaborate_program_with_resolver_and_modules(&ast, &resolver);
    let errors = result.expect_err("mismatched declaration must be rejected");
    assert!(errors.iter().any(|diagnostic| diagnostic.code == "MNE180"));
    assert!(resolutions.is_empty());
}

#[test]
fn null_resolver_finds_nothing_and_self_contained_profiles_still_work() {
    assert!(NullResolver.resolve("anything").is_none());
    let resolver = MapResolver::default();
    let program = elaborate(
        &resolver,
        "mncs 0.5;\nmodule solo;\nenum E { A, B }\nfn f(a: E) -> (r: E) { return a; }\n",
    )
    .expect("profile 0.5 module without imports still elaborates");
    assert_eq!(program.module, "solo");
}

/// Imported record types must be nameable in field and variant-payload type
/// positions of the importing module: cross-module composition requires a
/// local record or payload sum to reference an imported type.
#[test]
fn imported_types_are_nameable_in_field_and_payload_positions() {
    let resolver = MapResolver::default()
        .with(
            "lib.shapes",
            r#"mncs 0.6;

module lib.shapes;

record Point { x: i32, y: i32 }
"#,
        )
        .with(
            "app.study",
            r#"mncs 0.6;

module app.study;

use lib.shapes;

enum Mark { Placed { at: Point }, Cleared }

record Segment { from: Point, to: Point }

fn origin(segment: Segment) -> (result: Point) {
    return segment.from;
}
"#,
        );
    let ast = parse(&resolver.envelope("app.study", resolver.modules["app.study"].clone()))
        .ast
        .expect("consumer parses");
    let (result, _) = elaborate_program_with_resolver(&ast, &resolver);
    let program = result.expect("imported types participate in field positions");
    assert!(program.validate().valid);
    let segment = program
        .record_types
        .iter()
        .find(|record| record.name == "Segment")
        .expect("local record");
    assert_eq!(segment.fields.len(), 2);
}
