use std::collections::BTreeMap;

use mncs_compiler::{
    elaborate_program_with_resolver, elaborate_program_with_resolver_and_modules, ModuleResolver,
    NullResolver, ReferenceCompiler, ResolvedNameKind, SourceParseCache,
};
use mncs_model::{
    DeclarationFactReuseMiss, DeclarationVerificationFact, VerificationDependencyRole,
    VerificationFact, VerificationFactRequest, VerificationFactReuseMiss,
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
    assert!(
        parsed.is_valid(),
        "fixture parses: {:?}",
        parsed.diagnostics
    );
    let ast = parsed.ast.expect("fixture parses");
    match elaborate_program_with_resolver(&ast, resolver).0 {
        Ok(program) => Ok(program),
        Err(errors) => Err(errors.iter().map(|d| d.code.clone()).collect()),
    }
}

#[test]
fn profile_010_specializes_qualified_and_aliased_generic_calls_once() {
    let resolver = MapResolver::default().with(
        "lib.generic",
        r#"
mncs 0.10;
module lib.generic;
record Point { x: i64, y: i64 }
fn first<T, N: Nat>(xs: [T; N]) -> (result: T) { return xs[0]; }
"#,
    );
    let program = elaborate(
        &resolver,
        r#"
mncs 0.10;
module app.generic;
use lib.generic;
use lib.generic as g;
fn unqualified() -> (result: i64) {
    let p: lib.generic.Point = lib.generic.Point { x: 7, y: 8 };
    let xs: [lib.generic.Point; 2] = [p, p];
    return first<lib.generic.Point, 2>(xs).x;
}

fn qualified() -> (result: i64) {
    let p: lib.generic.Point = lib.generic.Point { x: 9, y: 10 };
    let xs: [lib.generic.Point; 2] = [p, p];
    return lib.generic.first<lib.generic.Point, 2>(xs).x;
}
fn aliased() -> (result: i64) {
    let p: lib.generic.Point = lib.generic.Point { x: 11, y: 12 };
    let xs: [lib.generic.Point; 2] = [p, p];
    return g.first<lib.generic.Point, 2>(xs).x;
}
"#,
    )
    .expect("generic module calls elaborate");

    assert_eq!(
        program
            .generic_specializations
            .iter()
            .filter(|record| record.canonical_args.contains("record:"))
            .count(),
        1
    );
    assert!(program.functions.iter().any(|function| {
        function.name.starts_with("first__spec_") && function.generic_params.is_empty()
    }));
    for function in &program.functions {
        if function.generic_params.is_empty() {
            if let Some(body) = &function.body {
                assert!(body
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .all(|op| {
                        !matches!(
                            &op.kind,
                            mncs_model::BodyOperationKind::Call { generic_args, .. }
                                if !generic_args.is_empty()
                        )
                    }));
            }
        }
    }
    let ssa = program.lower_to_ssa().expect("specialized program lowers");
    assert!(ssa.validate_lowering_boundary(&program).valid);
}

#[test]
fn profile_010_specializes_imported_generic_over_qualified_nominal_sequence_projection() {
    let resolver = MapResolver::default().with(
        "lib.status",
        r#"
mncs 0.10;
module lib.status;
enum Status { PASS, FAIL, UNKNOWN }
record Summary { status: Status, observed_count: i64 }
fn summarize<N: Nat>(statuses: [Status; N]) -> (result: Summary) {
    return Summary { status: statuses[0], observed_count: 1 };
}
"#,
    );
    let program = elaborate(
        &resolver,
        r#"
mncs 0.10;
module app.status;
use lib.status as s;
record Input { statuses: [s.Status; 2] }
fn demo(input: Input) -> (result: s.Summary) {
    return s.summarize<2>(input.statuses);
}
"#,
    )
    .expect("imported generic over a nominal sequence elaborates");

    assert!(program
        .functions
        .iter()
        .any(|function| function.name.starts_with("summarize__spec_")));
    let ssa = program
        .lower_to_ssa()
        .expect("imported nominal sequence specialization lowers");
    assert!(ssa.validate_lowering_boundary(&program).valid);
}

#[test]
fn profile_010_covers_zero_one_large_nested_bool_u64_and_empty_view_arguments() {
    let values = (1..=64)
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        r#"
mncs 0.10;
module app.generic_edges;
record Packet {{ payload: [i64; 2] }}

fn count<N: Nat>(xs: [i64; N]) -> (result: i64) {{
    iterate i over xs carrying total: i64 = 0 {{
        next total = total + xs[i];
    }}
    return total;
}}

fn identity<T>(value: T) -> (result: T) {{ return value; }}

fn first_or<N: Nat>(view: [i64; up_to N], fallback: i64) -> (result: i64) {{
    if view.len == 0 {{ return fallback; }}
    return view[0];
}}

fn demo() -> (result: i64) {{
    let zero: [i64; 0] = [];
    let one: [i64; 1] = [7];
    let large: [i64; 64] = [{values}];
    let nested: [i64; 2] = [3, 4];
    let flags: [bool; 1] = [true];
    let unsigned: u64 = 7;
    let small: [i64; 4] = [1, 2, 3, 4];
    let window: [i64; up_to 4] = small[0..0];
    let a: i64 = count<0>(zero);
    let b: i64 = count<1>(one);
    let c: i64 = count<64>(large);
    let d: [i64; 2] = identity<[i64; 2]>(nested);
    let e: [bool; 1] = identity<[bool; 1]>(flags);
    let f: u64 = identity<u64>(unsigned);
    let g: i64 = first_or<4>(window, 99);
    return a + b + c + d[0] + e[0] as i64 + f as i64 + g;
}}
"#
    );
    let resolver = MapResolver::default();
    let program = elaborate(&resolver, &source).expect("edge-case generic program elaborates");
    let canonical_args = program
        .generic_specializations
        .iter()
        .map(|record| record.canonical_args.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(canonical_args.iter().any(|args| args.contains("value:0")));
    assert!(canonical_args.iter().any(|args| args.contains("value:1")));
    assert!(canonical_args.iter().any(|args| args.contains("value:64")));
    assert!(canonical_args.iter().any(|args| args.contains("sequence<")));
    assert!(canonical_args.iter().any(|args| args.contains("bool")));
    assert!(canonical_args
        .iter()
        .any(|args| args.contains("unsigned:64")));
    assert!(program
        .functions
        .iter()
        .filter(|function| function.name.starts_with("count__spec_"))
        .all(|function| function.generic_params.is_empty()));
    let ssa = program
        .lower_to_ssa()
        .expect("edge-case specializations lower");
    assert!(ssa.validate_lowering_boundary(&program).valid);
}

#[test]
fn profile_010_refuses_unbounded_specialization_expansion() {
    let mut source = String::from("mncs 0.10;\nmodule app.generic_expansion;\n");
    for index in 0..130 {
        source.push_str(&format!("record R{index} {{ value: i64 }}\n"));
    }
    source.push_str("fn identity<T>(value: T) -> (result: T) { return value; }\n");
    source.push_str("fn demo() -> (result: i64) {\n");
    for index in 0..130 {
        source.push_str(&format!(
            "    let r{index}: R{index} = identity<R{index}>(R{index} {{ value: {index} }});\n"
        ));
    }
    source.push_str("    return 0;\n}\n");

    let errors = elaborate(&MapResolver::default(), &source)
        .expect_err("specialization expansion must fail closed");
    assert!(errors.contains(&"MNE227".to_owned()), "got {errors:?}");
}

#[test]
fn profile_010_rejects_invalid_or_ambiguous_generic_arguments() {
    let cases = [
        // `id(1)` used to be the missing-argument case; deterministic
        // inference (ENG-PRESSURE-0019) now solves `T = i64`, so the
        // refusal case is a parameter no argument can constrain.
        (
            "missing",
            "fn phantom<T>() -> (result: i64) { return 0; }\nfn demo() -> (result: i64) { return phantom(); }",
            "MNE220",
        ),
        (
            "count",
            "fn id<T>(value: T) -> (result: T) { return value; }\nfn demo() -> (result: i64) { return id<i64, 1>(1); }",
            "MNE221",
        ),
        (
            "wrong_kind",
            "fn width<N: Nat>(xs: [i64; N]) -> (result: i64) { return xs[0]; }\nfn demo() -> (result: i64) { let xs: [i64; 1] = [1]; return width<i64>(xs); }",
            "MNE222",
        ),
        (
            "non_constant",
            "fn width<N: Nat>(xs: [i64; N]) -> (result: i64) { return xs[0]; }\nfn demo(n: i64) -> (result: i64) { let xs: [i64; 1] = [1]; return width<n>(xs); }",
            "MNE224",
        ),
        (
            // The ceiling moved 64 -> 1024 (ENG-PRESSURE-0018); the
            // refusal now sits past the new bound.
            "out_of_range",
            "fn width<N: Nat>(xs: [i64; N]) -> (result: i64) { return xs[0]; }\nfn demo() -> (result: i64) { let xs: [i64; 1] = [1]; return width<1025>(xs); }",
            "MNE225",
        ),
        (
            "unsupported_kind",
            "fn higher<F: Type -> Type>(value: i64) -> (result: i64) { return value; }",
            "MNE229",
        ),
    ];
    for (label, body, expected) in cases {
        let source = format!("mncs 0.10;\nmodule app.generic_invalid_{label};\n{body}\n");
        let errors = elaborate(&MapResolver::default(), &source)
            .expect_err("invalid generic source must fail closed");
        assert!(
            errors.contains(&expected.to_owned()),
            "{label} expected {expected}, got {errors:?}"
        );
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
        host_grants: Vec::new(),
        type_arguments: Vec::new(),
        call_depth_budget: None,
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

fn duplicate_export_module(module: &str, function: &str) -> String {
    format!(
        "mncs 0.9;\nmodule {module};\nfn {function}(input: i64) -> (result: i64) {{ return input; }}\n"
    )
}

#[test]
fn profile_09_qualified_aliases_are_executable_and_inspectable() {
    let resolver = MapResolver::default()
        .with("lib.left", &duplicate_export_module("lib.left", "convert"))
        .with(
            "lib.right",
            &duplicate_export_module("lib.right", "convert"),
        );
    let source = r#"mncs 0.9;
module app.compose;
use lib.left as left;
use lib.right as right;
fn main(input: i64) -> (result: i64) {
    return left.convert(input);
}
"#;
    let root = resolver.envelope("root", source.to_owned());
    let front_end = ReferenceCompiler::default().front_end_with_resolver(root, &resolver);
    assert!(front_end.is_valid(), "{:#?}", front_end.diagnostics);
    let call_offset = source.find("left.convert").expect("qualified call");
    let resolution = front_end
        .name_resolutions
        .at_offset(call_offset + 2)
        .find(|resolution| resolution.kind == ResolvedNameKind::Function)
        .expect("qualified call resolution");
    assert_eq!(
        resolution.declaration_identity,
        Some(mncs_model::function_id("lib.left", "convert"))
    );
    assert_eq!(
        resolution.provenance,
        Some(mncs_model::ResolutionProvenance::Aliased)
    );
    let program = front_end.program.expect("program");
    let table = program.binding_table.as_ref().expect("binding table");
    assert!(table
        .references
        .iter()
        .any(|reference| reference.binding == resolution.binding.clone().expect("binding")));
    assert!(program
        .lower_to_ir()
        .expect("qualified call lowers to HIR")
        .functions
        .iter()
        .any(
            |function| function.semantic_identity == mncs_model::function_id("lib.left", "convert")
        ));
    assert!(program
        .lower_to_ssa()
        .expect("qualified call lowers to SSA")
        .functions
        .iter()
        .any(
            |function| function.semantic_identity == mncs_model::function_id("lib.left", "convert")
        ));
    let request = mncs_model::ExecutionRequest {
        schema_version: mncs_model::EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
        target: mncs_model::ExecutionTarget {
            module: "app.compose".to_owned(),
            function: "main".to_owned(),
        },
        arguments: vec![mncs_model::ExecutionValue::Integer {
            value: 9,
            ty: mncs_model::IntegerType {
                bits: 64,
                signed: true,
            },
        }],
        step_budget: 64,
        policy: mncs_model::ExecutionPolicy::default(),
        host_grants: Vec::new(),
        type_arguments: Vec::new(),
        call_depth_budget: None,
    };
    let body = mncs_model::execute(&program, &request);
    let ssa = mncs_model::execute_ssa(&program, &request);
    assert_eq!(body.status, mncs_model::ExecutionStatus::Returned);
    assert_eq!(ssa.status, mncs_model::ExecutionStatus::Returned);
    assert_eq!(body.returned, ssa.returned);
}

#[test]
fn profile_09_unqualified_duplicate_exports_fail_closed() {
    let resolver = MapResolver::default()
        .with("lib.left", &duplicate_export_module("lib.left", "convert"))
        .with(
            "lib.right",
            &duplicate_export_module("lib.right", "convert"),
        );
    let errors = elaborate(
        &resolver,
        "mncs 0.9;\nmodule app.ambiguous;\nuse lib.left;\nuse lib.right;\nfn main(input: i64) -> (result: i64) { return convert(input); }\n",
    )
    .unwrap_err();
    assert!(errors.contains(&"MNE131".to_owned()), "got {errors:?}");
}

#[test]
fn profile_09_unqualified_duplicate_nominal_exports_fail_closed() {
    let resolver = MapResolver::default()
        .with("lib.one", &duplicate_record_module("lib.one"))
        .with("lib.two", &duplicate_record_module("lib.two"));
    let errors = elaborate(
        &resolver,
        "mncs 0.9;\nmodule app.ambiguous_types;\nuse lib.one;\nuse lib.two;\nfn main(value: Box) -> (result: i64) { return value.value; }\n",
    )
    .unwrap_err();
    assert!(errors.contains(&"MNE105".to_owned()), "got {errors:?}");

    let program = elaborate(
        &resolver,
        "mncs 0.9;\nmodule app.local_type;\nuse lib.one;\nuse lib.two;\nrecord Box { value: i64 }\nfn main(value: Box) -> (result: i64) { return value.value; }\n",
    )
    .expect("a local nominal declaration wins over qualified imports");
    assert!(program.record_types.iter().any(|record| record.identity
        == mncs_model::record_type_id("app.local_type", "Box", &[("value", "i64")])));
}

#[test]
fn profile_018_retained_session_calls_imported_internal_recursive_record_function() {
    let resolver = MapResolver::default().with(
        "lib.machine",
        r#"
mncs 0.13;
module lib.machine;
enum Expr { Name { start: u64, end: u64 }, Integer { start: u64, end: u64, value: u64, overflow: bool }, Boolean { start: u64, end: u64, value: bool }, Binary { op: u64, left: Expr, right: Expr, start: u64, end: u64 }, Call { name_start: u64, name_end: u64, args: ExprList, start: u64, end: u64 }, Project { base: Expr, field_start: u64, field_end: u64, start: u64, end: u64, path: bool } }
enum ExprList { ENil, ECons { head: Expr, tail: ExprList } }
enum Stmt { Let { name_start: u64, name_end: u64, type_start: u64, type_end: u64, type_kind: u64, value: Expr, start: u64, end: u64 }, If { cond: Expr, then_b: StmtList, else_b: StmtList, start: u64, end: u64 }, Fail { mode_start: u64, mode_end: u64, start: u64, end: u64 }, Return { value: Expr, start: u64, end: u64 } }
enum StmtList { SNil, SCons { head: Stmt, tail: StmtList } }
record Frame { cond: Expr, cond_start: u64, cond_end: u64, then_list: StmtList, parent: StmtList, phase: u64 }
enum FrameList { YNil, YCons { head: Frame, tail: FrameList } }
enum FrameHolder { Wrap { frame: Frame } }
record BlockState { cursor: u64, ok: bool, err_start: u64, err_end: u64, list: StmtList, frames: FrameList, done: bool, mode: u64 }
fn block_step(s0: [byte; up_to 64], s1: [byte; up_to 64], s2: [byte; up_to 64], s3: [byte; up_to 64], total: u64, state: BlockState) -> (result: BlockState) { return BlockState { ..state, cursor: state.cursor + 1 }; }
fn has_frame(value: FrameHolder) -> (result: bool) { return true; }
fn parse(s0: [byte; up_to 64], s1: [byte; up_to 64], s2: [byte; up_to 64], s3: [byte; up_to 64], total: u64) -> (result: bool) {
    let expr: Expr = Expr.Name { start: 0, end: 1 };
    let stmt: Stmt = Stmt.Return { value: expr, start: 0, end: 1 };
    let body: StmtList = StmtList.SCons { head: stmt, tail: StmtList.SNil };
    let frame: Frame = Frame { cond: expr, cond_start: 0, cond_end: 1, then_list: body, parent: StmtList.SNil, phase: 0 };
    let frames: FrameList = FrameList.YCons { head: frame, tail: FrameList.YNil };
    let state: BlockState = BlockState { cursor: 0, ok: true, err_start: 0, err_end: 0, list: body, frames: frames, done: true, mode: 1 };
    let first: BlockState = block_step(s0, s1, s2, s3, total, state);
    let copied: BlockState = block_step(s0, s1, s2, s3, total, first);
    let held: bool = has_frame(FrameHolder.Wrap { frame: frame });
    return copied.done && held;
}
"#,
    ).with(
        "lib.facade",
        "mncs 0.13;\nmodule lib.facade;\nuse lib.machine as m;\nfn run(s0: [byte; up_to 64], s1: [byte; up_to 64], s2: [byte; up_to 64], s3: [byte; up_to 64], total: u64) -> (result: bool) { return m.parse(s0, s1, s2, s3, total); }\n",
    );
    let source = "mncs 0.18;\nmodule app.machine;\nuse lib.facade as facade;\nrecord Frame { marker: bool }\nfn marker() -> (result: bool) { let own: Frame = Frame { marker: true }; return own.marker; }\nfn main(s0: [byte; up_to 64], s1: [byte; up_to 64], s2: [byte; up_to 64], s3: [byte; up_to 64], total: u64) -> (result: bool) { return facade.run(s0, s1, s2, s3, total); }\n";
    let root = resolver.envelope("root", source.to_owned());
    let front_end = ReferenceCompiler::default().front_end_with_resolver(root, &resolver);
    assert!(front_end.is_valid(), "{:#?}", front_end.diagnostics);
    let program = front_end.program.expect("linked program");
    let request = mncs_model::ExecutionRequest {
        schema_version: mncs_model::EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
        target: mncs_model::ExecutionTarget {
            module: "app.machine".to_owned(),
            function: "main".to_owned(),
        },
        arguments: vec![
            mncs_model::ExecutionValue::Sequence {
                values: Vec::new().into(),
            },
            mncs_model::ExecutionValue::Sequence {
                values: Vec::new().into(),
            },
            mncs_model::ExecutionValue::Sequence {
                values: Vec::new().into(),
            },
            mncs_model::ExecutionValue::Sequence {
                values: Vec::new().into(),
            },
            mncs_model::ExecutionValue::Integer {
                value: 0,
                ty: mncs_model::IntegerType {
                    bits: 64,
                    signed: false,
                },
            },
        ],
        step_budget: 256,
        policy: mncs_model::ExecutionPolicy::default(),
        host_grants: Vec::new(),
        type_arguments: Vec::new(),
        call_depth_budget: None,
    };
    let session = mncs_model::BodyExecutionSession::new(&program);
    let result = session.execute(&request);
    assert_eq!(
        result.status,
        mncs_model::ExecutionStatus::Returned,
        "{result:?}"
    );
    assert_eq!(
        result.returned,
        vec![mncs_model::ExecutionValue::Boolean { value: true }]
    );
    let ssa = mncs_model::execute_ssa(&program, &request);
    assert_eq!(ssa.status, mncs_model::ExecutionStatus::Returned, "{ssa:?}");
    assert_eq!(
        ssa.returned,
        vec![mncs_model::ExecutionValue::Boolean { value: true }]
    );
}

#[test]
fn profile_09_alias_collisions_and_missing_members_are_diagnostics() {
    let resolver = MapResolver::default()
        .with("lib.left", &duplicate_export_module("lib.left", "convert"))
        .with(
            "lib.right",
            &duplicate_export_module("lib.right", "convert"),
        );
    let duplicate_alias = elaborate(
        &resolver,
        "mncs 0.9;\nmodule app.aliases;\nuse lib.left as same;\nuse lib.right as same;\nfn main(input: i64) -> (result: i64) { return same.convert(input); }\n",
    )
    .unwrap_err();
    assert!(duplicate_alias.contains(&"MNE181".to_owned()));

    let missing_member = elaborate(
        &resolver,
        "mncs 0.9;\nmodule app.missing;\nuse lib.left as left;\nfn main(input: i64) -> (result: i64) { return left.unknown(input); }\n",
    )
    .unwrap_err();
    assert!(missing_member.contains(&"MNE131".to_owned()));
}

fn duplicate_record_module(module: &str) -> String {
    format!(
        "mncs 0.5;\nmodule {module};\nrecord Box {{ value: i64 }}\nfn make(value: i64) -> (result: Box) {{ return Box {{ value: value }}; }}\nfn get(value: Box) -> (result: i64) {{ return value.value; }}\n"
    )
}

#[test]
fn profile_09_qualified_type_identities_survive_duplicate_record_names() {
    let resolver = MapResolver::default()
        .with("lib.one", &duplicate_record_module("lib.one"))
        .with("lib.two", &duplicate_record_module("lib.two"));
    let source = "mncs 0.9;\nmodule app.records;\nuse lib.one as one;\nuse lib.two as two;\nfn main(value: i64) -> (result: i64) { let left: one.Box = one.make(value); let right: two.Box = two.make(value); return one.get(left) + two.get(right); }\n";
    let root = resolver.envelope("root", source.to_owned());
    let front_end = ReferenceCompiler::default().front_end_with_resolver(root, &resolver);
    assert!(front_end.is_valid(), "{:#?}", front_end.diagnostics);
    let program = front_end.program.expect("program");
    let one_box = program
        .record_types
        .iter()
        .find(|record| {
            record.identity == mncs_model::record_type_id("lib.one", "Box", &[("value", "i64")])
        })
        .expect("first imported record identity");
    let two_box = program
        .record_types
        .iter()
        .find(|record| {
            record.identity == mncs_model::record_type_id("lib.two", "Box", &[("value", "i64")])
        })
        .expect("second imported record identity");
    assert_ne!(one_box.identity, two_box.identity);
    let calls = program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .and_then(|function| function.body.as_ref())
        .into_iter()
        .flat_map(|body| body.blocks.iter())
        .flat_map(|block| block.operations.iter())
        .filter_map(|operation| match &operation.kind {
            mncs_model::BodyOperationKind::Call { function, .. } => Some(function.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(calls.contains(&mncs_model::function_id("lib.one", "get")));
    assert!(calls.contains(&mncs_model::function_id("lib.two", "get")));
}

#[test]
fn profile_09_canonicalizes_imported_nominal_record_fields() {
    let resolver = MapResolver::default().with("lib.dep", &duplicate_record_module("lib.dep"));
    let program = elaborate(
        &resolver,
        "mncs 0.9;\nmodule app.nested_records;\nuse lib.dep as dep;\nrecord Wrapper { value: dep.Box }\nfn wrap(value: dep.Box) -> (result: Wrapper) { return Wrapper { value: value }; }\nfn unwrap(value: Wrapper) -> (result: i64) { return value.value.value; }\n",
    )
    .expect("imported nominal record fields elaborate");

    let box_identity = mncs_model::record_type_id("lib.dep", "Box", &[("value", "i64")]);
    let wrapper = program
        .record_types
        .iter()
        .find(|record| record.name == "Wrapper")
        .expect("wrapper record");
    assert_eq!(wrapper.fields[0].field_type, box_identity.0);
    assert_eq!(
        wrapper.identity,
        mncs_model::record_type_id(
            "app.nested_records",
            "Wrapper",
            &[("value", &box_identity.0)]
        )
    );
}

#[test]
fn profile_09_qualified_finite_identities_survive_duplicate_names() {
    let module = |name: &str| {
        format!(
            "mncs 0.5;\nmodule {name};\nenum Status {{ Ready, Halted }}\nfn ready() -> (result: Status) {{ return Status.Ready; }}\n"
        )
    };
    let resolver = MapResolver::default()
        .with("lib.one", &module("lib.one"))
        .with("lib.two", &module("lib.two"));
    let source = "mncs 0.9;\nmodule app.finite;\nuse lib.one as one;\nuse lib.two as two;\nfn make() -> (result: one.Status) { return one.Status.Ready; }\nfn main() -> (result: i64) { let status: one.Status = one.ready(); return match status { one.Status.Ready => 1, one.Status.Halted => 0, }; }\n";
    let root = resolver.envelope("root", source.to_owned());
    let front_end = ReferenceCompiler::default().front_end_with_resolver(root, &resolver);
    assert!(front_end.is_valid(), "{:#?}", front_end.diagnostics);
    let program = front_end.program.expect("program");
    assert!(program
        .finite_types
        .iter()
        .any(|finite| finite.identity == mncs_model::finite_type_id("lib.one", "Status")));
    assert!(program
        .finite_types
        .iter()
        .any(|finite| finite.identity == mncs_model::finite_type_id("lib.two", "Status")));
    assert!(program
        .binding_table
        .as_ref()
        .expect("binding table")
        .bindings
        .iter()
        .any(|binding| binding.declaration
            == mncs_model::finite_variant_id("lib.one", "Status", "Ready")));
}

#[test]
fn profile_010_allows_local_record_sequences_inside_record_fields() {
    let program = elaborate(
        &MapResolver::default(),
        r#"
mncs 0.10;
module app.record_sequence_field;
record Project { id: i64 }
record Atlas { projects: [Project; 2] }
fn make() -> (result: Atlas) {
    let project: Project = Project { id: 7 };
    let projects: [Project; 2] = [project, project];
    return Atlas { projects: projects };
}
"#,
    )
    .expect("local record sequence field elaborates");
    let atlas = program
        .record_types
        .iter()
        .find(|record| record.name == "Atlas")
        .expect("Atlas record");
    assert_eq!(atlas.fields.len(), 1);
    assert!(atlas.fields[0].field_type.contains("Project"));
}

#[test]
fn profile_09_qualified_record_sequence_elements_keep_declaring_identity() {
    let resolver = MapResolver::default().with(
        "lib.geom",
        "mncs 0.9;\nmodule lib.geom;\nrecord Point { column: i64, row: i64 }\n",
    );
    let program = elaborate(
        &resolver,
        r#"
mncs 0.9;
module app.windows;
use lib.geom as geom;
fn first_column(points: [geom.Point; 2]) -> (result: i64) {
    return points[0].column;
}
"#,
    )
    .expect("qualified record sequence elements elaborate");
    let function = program
        .functions
        .iter()
        .find(|function| function.name == "first_column")
        .expect("first_column");
    let point_id =
        mncs_model::record_type_id("lib.geom", "Point", &[("column", "i64"), ("row", "i64")]);
    assert_eq!(
        function.inputs[0].value_type,
        format!("[{}; 2]", point_id.0)
    );
    let parameter = &function.body.as_ref().expect("body").parameters[0];
    match &parameter.ty {
        mncs_model::BodyType::Sequence { element, bound } => {
            assert_eq!(*bound, mncs_model::SequenceBound::Exact(2));
            match element.as_ref() {
                mncs_model::BodyType::Record { name, identity } => {
                    assert_eq!(name, "Point");
                    assert_eq!(identity, &point_id);
                }
                other => panic!("expected record element, got {other:?}"),
            }
        }
        other => panic!("expected sequence parameter, got {other:?}"),
    }
}

#[test]
fn profile_09_keeps_nested_local_field_projection_distinct_from_qualified_constructors() {
    let resolver = MapResolver::default();
    let program = elaborate(
        &resolver,
        "mncs 0.9;\nmodule app.paths;\nrecord Inner { value: i64 }\nrecord Outer { inner: Inner }\nfn read(value: Outer) -> (result: i64) { return value.inner.value; }\n",
    )
    .expect("nested local field paths remain executable");
    let body = program
        .functions
        .iter()
        .find(|function| function.name == "read")
        .and_then(|function| function.body.as_ref())
        .expect("read body");
    assert_eq!(
        body.blocks
            .iter()
            .flat_map(|block| block.operations.iter())
            .filter(|operation| matches!(
                operation.kind,
                mncs_model::BodyOperationKind::RecordProject { .. }
            ))
            .count(),
        2
    );
}

#[test]
fn profile_09_import_discovery_order_does_not_change_binding_identity() {
    let resolver = MapResolver::default()
        .with("lib.left", &duplicate_export_module("lib.left", "convert"))
        .with(
            "lib.right",
            &duplicate_export_module("lib.right", "convert"),
        );
    let first = elaborate(
        &resolver,
        "mncs 0.9;\nmodule app.order;\nuse lib.left as left;\nuse lib.right as right;\nfn main(input: i64) -> (result: i64) { return left.convert(input); }\n",
    )
    .expect("first order elaborates");
    let second = elaborate(
        &resolver,
        "mncs 0.9;\nmodule app.order;\nuse lib.right as right;\nuse lib.left as left;\nfn main(input: i64) -> (result: i64) { return left.convert(input); }\n",
    )
    .expect("second order elaborates");
    assert_eq!(
        first.content_fingerprint().expect("first canonical form"),
        second.content_fingerprint().expect("second canonical form")
    );
    let first_binding = first
        .binding_table
        .as_ref()
        .expect("first binding table")
        .references
        .iter()
        .find(|reference| reference.provenance == mncs_model::ResolutionProvenance::Aliased)
        .expect("first alias witness");
    let second_binding = second
        .binding_table
        .as_ref()
        .expect("second binding table")
        .references
        .iter()
        .find(|reference| reference.provenance == mncs_model::ResolutionProvenance::Aliased)
        .expect("second alias witness");
    assert_eq!(first_binding.binding, second_binding.binding);
}

#[test]
fn profile_09_source_offsets_do_not_rename_semantic_bindings() {
    let resolver =
        MapResolver::default().with("lib.left", &duplicate_export_module("lib.left", "convert"));
    let first = elaborate(
        &resolver,
        "mncs 0.9;\nmodule app.offsets;\nuse lib.left as left;\nfn main(input: i64) -> (result: i64) { return left.convert(input); }\n",
    )
    .expect("first source elaborates");
    let second = elaborate(
        &resolver,
        "mncs 0.9;\n\n// harmless source movement\nmodule app.offsets;\nuse lib.left as left;\n\nfn main(input: i64) -> (result: i64) {\n    return left.convert(input);\n}\n",
    )
    .expect("second source elaborates");
    assert_eq!(
        first.content_fingerprint().expect("first canonical form"),
        second.content_fingerprint().expect("second canonical form")
    );
    let first_reference = first
        .binding_table
        .as_ref()
        .expect("first binding table")
        .references
        .iter()
        .find(|reference| reference.provenance == mncs_model::ResolutionProvenance::Aliased)
        .expect("first alias reference");
    let second_reference = second
        .binding_table
        .as_ref()
        .expect("second binding table")
        .references
        .iter()
        .find(|reference| reference.provenance == mncs_model::ResolutionProvenance::Aliased)
        .expect("second alias reference");
    assert_eq!(first_reference.identity, second_reference.identity);
    assert_eq!(first_reference.binding, second_reference.binding);
}

#[test]
fn profile_09_local_unqualified_binding_wins_without_hiding_qualified_imports() {
    let resolver = MapResolver::default()
        .with("lib.left", &duplicate_export_module("lib.left", "convert"))
        .with(
            "lib.right",
            &duplicate_export_module("lib.right", "convert"),
        );
    let source = "mncs 0.9;\nmodule app.local;\nuse lib.left as left;\nuse lib.right as right;\nfn convert(input: i64) -> (result: i64) { return input + 1; }\nfn main(input: i64) -> (result: i64) { return convert(input); }\n";
    let root = resolver.envelope("root", source.to_owned());
    let front_end = ReferenceCompiler::default().front_end_with_resolver(root, &resolver);
    assert!(front_end.is_valid(), "{:#?}", front_end.diagnostics);
    let main_offset = source.rfind("convert(input)").expect("local call");
    let resolution = front_end
        .name_resolutions
        .at_offset(main_offset)
        .find(|resolution| resolution.kind == ResolvedNameKind::Function)
        .expect("local call resolution");
    assert_eq!(
        resolution.declaration_identity,
        Some(mncs_model::function_id("app.local", "convert"))
    );
    assert_eq!(
        resolution.provenance,
        Some(mncs_model::ResolutionProvenance::Local)
    );
    let qualified = front_end
        .program
        .expect("program")
        .binding_table
        .expect("binding table");
    assert!(qualified
        .bindings
        .iter()
        .any(|binding| binding.declaration == mncs_model::function_id("lib.left", "convert")));
}

fn shapes_module() -> String {
    "mncs 0.10;\nmodule lib.shapes;\nrecord Point { x: i64, y: i64 }\nrecord Box { p: Point }\n"
        .to_owned()
}

fn other_point_module() -> String {
    "mncs 0.10;\nmodule lib.other;\nrecord Point { tag: i64 }\n".to_owned()
}

fn execute_main(program: &mncs_model::Program, module: &str) -> mncs_model::ExecutionResult {
    let request = mncs_model::ExecutionRequest {
        schema_version: mncs_model::EXECUTION_REQUEST_SCHEMA_VERSION.to_owned(),
        target: mncs_model::ExecutionTarget {
            module: module.to_owned(),
            function: "main".to_owned(),
        },
        arguments: Vec::new(),
        step_budget: 256,
        policy: mncs_model::ExecutionPolicy::default(),
        host_grants: Vec::new(),
        type_arguments: Vec::new(),
        call_depth_budget: None,
    };
    mncs_model::execute(program, &request)
}

fn integer_returned(result: &mncs_model::ExecutionResult) -> i128 {
    assert_eq!(result.status, mncs_model::ExecutionStatus::Returned);
    assert_eq!(result.returned.len(), 1);
    match &result.returned[0] {
        mncs_model::ExecutionValue::Integer { value, .. } => *value,
        other => panic!("expected integer return, got {other:?}"),
    }
}

/// A two-segment `alias.Record { ... }` parses as a finite-variant
/// constructor; elaboration must retry the joined spelling in the record
/// namespace (P-COMMONS-03). The constructed value executes and the
/// binding carries qualified provenance with the declaring identity.
#[test]
fn profile_10_qualified_record_construction_executes() {
    let resolver = MapResolver::default()
        .with("lib.shapes", &shapes_module())
        .with("lib.other", &other_point_module());
    let source = "mncs 0.10;\nmodule app.qualified_record;\nuse lib.shapes as shapes;\nfn main() -> (result: i64) {\n    let made: shapes.Point = shapes.Point { x: 1, y: 2 };\n    return made.x + made.y;\n}\n";
    let root = resolver.envelope("root", source.to_owned());
    let front_end = ReferenceCompiler::default().front_end_with_resolver(root, &resolver);
    assert!(front_end.is_valid(), "{:#?}", front_end.diagnostics);
    let expected_identity =
        mncs_model::record_type_id("lib.shapes", "Point", &[("x", "i64"), ("y", "i64")]);
    assert!(
        front_end
            .name_resolutions
            .resolutions
            .iter()
            .any(
                |resolution| resolution.declaration_identity.as_ref() == Some(&expected_identity)
                    && resolution.provenance == Some(mncs_model::ResolutionProvenance::Aliased)
            ),
        "aliased record-type resolution missing: {:#?}",
        front_end.name_resolutions
    );
    let program = front_end.program.expect("program");
    assert_eq!(
        integer_returned(&execute_main(&program, "app.qualified_record")),
        3
    );
}

/// Colliding short record names in two imported modules stay distinct:
/// each alias route constructs its own declaring identity.
#[test]
fn profile_10_qualified_record_construction_disambiguates_colliding_names() {
    let resolver = MapResolver::default()
        .with("lib.shapes", &shapes_module())
        .with("lib.other", &other_point_module());
    let source = "mncs 0.10;\nmodule app.colliding;\nuse lib.shapes as shapes;\nuse lib.other as other;\nfn main() -> (result: i64) {\n    let first: shapes.Point = shapes.Point { x: 10, y: 20 };\n    let second: other.Point = other.Point { tag: 7 };\n    return first.x + first.y + second.tag;\n}\n";
    let program = elaborate(&resolver, source).expect("colliding routes elaborate");
    assert_eq!(
        integer_returned(&execute_main(&program, "app.colliding")),
        37
    );
}

/// Qualified construction nests: a record field holding another
/// qualified record elaborates and executes.
#[test]
fn profile_10_qualified_record_construction_nests() {
    let resolver = MapResolver::default().with("lib.shapes", &shapes_module());
    let source = "mncs 0.10;\nmodule app.nested;\nuse lib.shapes as shapes;\nfn main() -> (result: i64) {\n    let made: shapes.Box = shapes.Box { p: shapes.Point { x: 4, y: 5 } };\n    return made.p.x + made.p.y;\n}\n";
    let program = elaborate(&resolver, source).expect("nested construction elaborates");
    assert_eq!(integer_returned(&execute_main(&program, "app.nested")), 9);
}

/// Unknown qualified records still fail closed, and finite payload
/// construction keeps priority over the record fallback.
#[test]
fn profile_10_qualified_record_misses_fail_closed() {
    let resolver = MapResolver::default().with("lib.shapes", &shapes_module());
    let unknown = elaborate(
        &resolver,
        "mncs 0.10;\nmodule app.miss;\nuse lib.shapes as shapes;\nfn main() -> (result: i64) {\n    let made: shapes.Point = shapes.Nope { x: 1 };\n    return made.x;\n}\n",
    )
    .expect_err("unknown qualified record still fails");
    assert!(
        unknown.contains(&"MNE123".to_owned()),
        "expected MNE123, got {unknown:?}"
    );
    let finite_first = elaborate(
        &resolver,
        "mncs 0.10;\nmodule app.finite;\nenum Sig { Yes { v: i64 }, No }\nfn main() -> (result: i64) {\n    let made: Sig = Sig.Yes { v: 7 };\n    return match made { Sig.Yes { v } => v, Sig.No => 0 };\n}\n",
    );
    assert!(finite_first.is_ok(), "{finite_first:?}");
}

#[test]
fn profile_09_transitive_visibility_keeps_the_declaring_identity() {
    let resolver = MapResolver::default()
        .with("lib.base", &duplicate_export_module("lib.base", "convert"))
        .with(
            "lib.facade",
            "mncs 0.9;\nmodule lib.facade;\nuse lib.base as base;\nfn facade(input: i64) -> (result: i64) { return base.convert(input); }\n",
        );
    let source = "mncs 0.9;\nmodule app.transitive;\nuse lib.facade as facade;\nfn main(input: i64) -> (result: i64) { return facade.convert(input); }\n";
    let root = resolver.envelope("root", source.to_owned());
    let front_end = ReferenceCompiler::default().front_end_with_resolver(root, &resolver);
    assert!(front_end.is_valid(), "{:#?}", front_end.diagnostics);
    assert_eq!(
        front_end
            .name_resolutions
            .resolutions
            .iter()
            .filter_map(|resolution| resolution.declaration_identity.as_ref())
            .filter(|identity| **identity == mncs_model::function_id("lib.base", "convert"))
            .count(),
        1
    );
}

#[test]
fn imported_nominal_types_resolve_in_signatures_projection_and_match() {
    let machine = "mncs 0.6;\nmodule lib.machine;\nenum Phase { stopped, playing, paused }\nenum Evt { play, pause, stop }\nrecord Transport { phase: Phase, armed: bool }\nfn step(state: Transport, event: Evt) -> (result: Transport) {\n    return match event {\n        play => Transport { phase: Phase.playing, armed: state.armed },\n        pause => Transport { phase: Phase.paused, armed: state.armed },\n        stop => Transport { phase: Phase.stopped, armed: false },\n    };\n}\nfn is_playing(state: Transport) -> (result: bool) {\n    return match state.phase {\n        playing => true,\n        stopped => false,\n        paused => false,\n    };\n}\n";
    let resolver = MapResolver::default().with("lib.machine", machine);
    let program = elaborate(
        &resolver,
        "mncs 0.6;\nmodule app.transport;\nuse lib.machine;\nfn launch(armed: bool) -> (result: Transport) {\n    let init: Transport = Transport { phase: Phase.stopped, armed: armed };\n    return step(init, Evt.play);\n}\nfn describe(state: Transport) -> (result: bool) {\n    return match state.phase {\n        playing => true,\n        stopped => false,\n        paused => false,\n    };\n}\n",
    )
    .expect("imported nominal types elaborate");
    let launch = program
        .functions
        .iter()
        .find(|function| function.name == "launch")
        .expect("launch survives elaboration");
    let result_ty = launch.outputs.first().expect("launch declares a result");
    let transport = program
        .record_types
        .iter()
        .find(|record| record.name == "Transport")
        .expect("imported record is registered");
    assert_eq!(result_ty.value_type, transport.identity.0);
    assert!(
        transport.identity.0.contains("lib.machine"),
        "imported record keeps its declaring identity, got {}",
        transport.identity.0
    );
}

fn elaborate_full(
    resolver: &MapResolver,
    text: &str,
) -> Result<mncs_model::Program, Vec<mncs_syntax::SourceDiagnostic>> {
    let envelope = resolver.envelope("root", text.to_owned());
    let parsed = parse(&envelope);
    assert!(
        parsed.is_valid(),
        "fixture parses: {:?}",
        parsed.diagnostics
    );
    let ast = parsed.ast.expect("fixture parses");
    elaborate_program_with_resolver(&ast, resolver).0
}

/// INGEST-P-007: a leaf parse failure (the ingest `over`-field shape)
/// must surface through the root compile with the leaf diagnostic
/// preserved — module identity, original code, message, and leaf-relative
/// span — not reduced to bare codes.
#[test]
fn imported_parse_failure_preserves_leaf_diagnostics() {
    let resolver = MapResolver::default().with(
        "mncs.ingest.parse",
        "mncs 0.10;\nmodule mncs.ingest.parse;\nrecord Transfer { over: u64 }\nfn get(t: Transfer) -> (result: u64) {\n    return t.over;\n}\n",
    );
    let errors = elaborate_full(
        &resolver,
        "mncs 0.10;\nmodule app.root;\nuse mncs.ingest.parse;\nfn entry(t: u64) -> (r: u64) { return t; }\n",
    )
    .unwrap_err();
    assert_eq!(
        errors.len(),
        1,
        "one import-edge diagnostic, got {errors:?}"
    );
    let outer = &errors[0];
    assert_eq!(
        outer.code, "MNE172",
        "outer context is preserved, got {outer:?}"
    );
    assert!(
        outer.message.contains("mncs.ingest.parse") && outer.message.contains("failed to parse"),
        "outer names the module and the failure, got {}",
        outer.message
    );
    assert_eq!(
        (outer.span.line, outer.span.column),
        (3, 5),
        "outer span is the root use-site, got {:?}",
        outer.span
    );
    assert!(
        !outer.related.is_empty(),
        "leaf diagnostics must travel with the wrapper, got {outer:?}"
    );
    assert!(
        outer
            .related
            .iter()
            .any(|diagnostic| diagnostic.code == "MNP127"),
        "leaf MNP127 is preserved, got {:?}",
        outer
            .related
            .iter()
            .map(|diagnostic| &diagnostic.code)
            .collect::<Vec<_>>()
    );
    for diagnostic in &outer.related {
        assert!(
            !diagnostic.message.is_empty(),
            "leaf message is preserved, got {diagnostic:?}"
        );
        assert!(
            diagnostic.span.line >= 1,
            "leaf span is preserved, got {diagnostic:?}"
        );
    }
}

/// INGEST-P-007: the same preservation holds for leaf elaboration
/// failures, not just parse failures.
#[test]
fn imported_elaboration_failure_preserves_leaf_diagnostics() {
    let resolver = MapResolver::default().with(
        "lib.broken",
        "mncs 0.6;\nmodule lib.broken;\nfn get(t: u64) -> (result: u64) {\n    return missing;\n}\n",
    );
    let errors = elaborate_full(
        &resolver,
        "mncs 0.6;\nmodule app.root;\nuse lib.broken;\nfn entry(t: u64) -> (r: u64) { return t; }\n",
    )
    .unwrap_err();
    assert_eq!(
        errors.len(),
        1,
        "one import-edge diagnostic, got {errors:?}"
    );
    let outer = &errors[0];
    assert_eq!(
        outer.code, "MNE172",
        "outer context is preserved, got {outer:?}"
    );
    assert!(
        outer.message.contains("lib.broken") && outer.message.contains("failed to elaborate"),
        "outer names the module and the failure, got {}",
        outer.message
    );
    assert!(
        !outer.related.is_empty(),
        "leaf diagnostics must travel with the wrapper, got {outer:?}"
    );
    assert!(
        outer
            .related
            .iter()
            .all(|diagnostic| diagnostic.code.starts_with("MNE")),
        "leaf elaboration codes are preserved, got {:?}",
        outer
            .related
            .iter()
            .map(|diagnostic| &diagnostic.code)
            .collect::<Vec<_>>()
    );
}

/// INGEST-P-007: a failure two hops down records every import edge —
/// root -> mid -> leaf — so the consumer can trace the causal path
/// without recompiling each leaf as its own root.
#[test]
fn nested_import_failure_records_every_edge() {
    let resolver = MapResolver::default()
        .with(
            "mid.level",
            "mncs 0.10;\nmodule mid.level;\nuse mncs.ingest.parse;\nfn mid(t: u64) -> (r: u64) { return t; }\n",
        )
        .with(
            "mncs.ingest.parse",
            "mncs 0.10;\nmodule mncs.ingest.parse;\nrecord Transfer { over: u64 }\nfn get(t: Transfer) -> (result: u64) {\n    return t.over;\n}\n",
        );
    let errors = elaborate_full(
        &resolver,
        "mncs 0.10;\nmodule app.root;\nuse mid.level;\nfn entry(t: u64) -> (r: u64) { return t; }\n",
    )
    .unwrap_err();
    assert_eq!(
        errors.len(),
        1,
        "one import-edge diagnostic, got {errors:?}"
    );
    let outer = &errors[0];
    assert_eq!(outer.code, "MNE172", "got {outer:?}");
    assert!(
        outer.message.contains("mid.level") && outer.message.contains("failed to load"),
        "outer names the direct edge, got {}",
        outer.message
    );
    assert_eq!(outer.related.len(), 1, "one mid-level cause, got {outer:?}");
    let mid = &outer.related[0];
    assert_eq!(
        mid.code, "MNE172",
        "mid edge keeps its wrapper, got {mid:?}"
    );
    assert!(
        mid.message.contains("mncs.ingest.parse"),
        "mid names the leaf edge, got {}",
        mid.message
    );
    assert!(
        mid.related
            .iter()
            .any(|diagnostic| diagnostic.code == "MNP127"),
        "leaf parse code survives both hops, got {mid:?}"
    );
}

#[test]
fn projection_from_a_non_record_value_is_rejected() {
    let resolver = MapResolver::default();
    let errors = elaborate(
        &resolver,
        "mncs 0.6;\nmodule app.projection;\nfn f(value: i64) -> (result: i64) { return value.phase; }\n",
    )
    .unwrap_err();
    assert!(errors.contains(&"MNE161".to_owned()), "got {errors:?}");
}

#[test]
fn linked_frontend_reuses_language_facts_with_per_module_profiles_and_fresh_locations() {
    let resolver = MapResolver::default().with(
        "lib.cached",
        "mncs 0.18;\nmodule lib.cached.v1;\nfn pass(value: u64) -> (result: u64) { return value; }\n",
    );
    let compiler = ReferenceCompiler::default();
    let request = |prior_facts, prior_declaration_facts| VerificationFactRequest {
        language_profile: "0.19".to_owned(),
        subject_language_profiles: BTreeMap::new(),
        prior_facts,
        prior_declaration_facts,
        source_location_identities: BTreeMap::new(),
    };
    let source = "mncs 0.19;\nmodule app.cached;\nuse lib.cached;\nfn entry(value: u64) -> (result: u64) { return lib.cached.pass(value); }\n";
    let first_envelope = resolver.envelope("root", source.to_owned());
    let first = compiler.front_end_with_verified_facts(
        first_envelope.clone(),
        &resolver,
        &[],
        &request(Vec::new(), Vec::new()),
    );
    assert!(first.is_valid(), "{:?}", first.diagnostics);
    assert_eq!(first.verification_facts.len(), 2);
    assert_eq!(
        first
            .module_resolutions
            .iter()
            .find(|module| module.declared_module == "lib.cached.v1")
            .map(|module| module.source_profile.as_str()),
        Some("0.18")
    );

    let edited_source = format!("// comment-only edit\n{source}");
    let second_envelope = resolver.envelope("root", edited_source);
    let second = compiler.front_end_with_verified_facts(
        second_envelope.clone(),
        &resolver,
        &[],
        &request(
            first.verification_facts,
            first.declaration_verification_facts,
        ),
    );
    assert!(second.is_valid(), "{:?}", second.diagnostics);
    assert_eq!(
        second
            .verification_fact_reuse
            .as_ref()
            .unwrap()
            .reused
            .len(),
        2
    );
    assert!(second
        .verification_fact_reuse
        .as_ref()
        .unwrap()
        .recomputed
        .is_empty());
    assert_eq!(
        second.declaration_fact_reuse.as_ref().unwrap().reused.len(),
        2
    );
    assert!(second
        .declaration_fact_reuse
        .as_ref()
        .unwrap()
        .recomputed
        .is_empty());
    let imported_source_identity = resolver
        .envelope("lib.cached", resolver.modules["lib.cached"].clone())
        .identity;
    assert!(second.verification_facts.iter().all(|fact| {
        let expected = if fact.subject.as_str().contains("lib.cached") {
            imported_source_identity.as_str()
        } else {
            second_envelope.identity.as_str()
        };
        fact.source_location_identity.as_deref() == Some(expected)
    }));
}

#[test]
fn linked_frontend_reuses_unchanged_import_parses_after_cache_restart() {
    let library_source =
        "mncs 0.19;\nmodule lib.parse_cache;\nfn pass(value: u64) -> (result: u64) { return value; }\n";
    let resolver = MapResolver::default().with("lib.parse_cache", library_source);
    let compiler = ReferenceCompiler::default();
    let request = |prior_facts, prior_declaration_facts| VerificationFactRequest {
        language_profile: "0.19".to_owned(),
        subject_language_profiles: BTreeMap::new(),
        prior_facts,
        prior_declaration_facts,
        source_location_identities: BTreeMap::new(),
    };
    let source = "mncs 0.19;\nmodule app.parse_cache;\nuse lib.parse_cache;\nfn entry(value: u64) -> (result: u64) { return lib.parse_cache.pass(value); }\n";
    let mut parse_cache = SourceParseCache::default();
    let first = compiler.front_end_with_verified_facts_and_source_parse_cache(
        resolver.envelope("root", source.to_owned()),
        &resolver,
        &[],
        &request(Vec::new(), Vec::new()),
        &mut parse_cache,
    );
    assert!(first.is_valid(), "{:?}", first.diagnostics);
    let first_cache_observation = first
        .source_parse_cache_reuse
        .as_ref()
        .expect("parse cache observation");
    assert_eq!(first_cache_observation.parsed_source_identities.len(), 2);
    assert_eq!(first_cache_observation.reused_source_identities.len(), 0);
    assert_eq!(first_cache_observation.published_entry_count, 2);

    // A fresh process restores only the caller-owned Language parse cache.
    let serialized_cache = serde_json::to_vec(&parse_cache).expect("parse cache serializes");
    let mut parse_cache: SourceParseCache =
        serde_json::from_slice(&serialized_cache).expect("parse cache restores");

    let edited_source = format!("// root snapshot comment edit\n{source}");
    let second = compiler.front_end_with_verified_facts_and_source_parse_cache(
        resolver.envelope("root", edited_source),
        &resolver,
        &[],
        &request(
            first.verification_facts,
            first.declaration_verification_facts,
        ),
        &mut parse_cache,
    );
    assert!(second.is_valid(), "{:?}", second.diagnostics);
    let observation = second
        .source_parse_cache_reuse
        .as_ref()
        .expect("parse cache observation");
    assert_eq!(observation.reused_source_identities.len(), 1);
    assert_eq!(
        observation.reused_source_identities[0],
        resolver
            .envelope("lib.parse_cache", library_source.to_owned())
            .identity
    );
    assert_eq!(observation.parsed_source_identities.len(), 1);
    assert_eq!(observation.published_entry_count, 1);
    assert_eq!(
        second
            .verification_fact_reuse
            .as_ref()
            .expect("semantic fact observation")
            .reused
            .len(),
        2
    );
}

#[test]
fn linked_frontend_reuses_serialized_verification_facts_and_rechecks_tampered_witnesses() {
    let library_source =
        "mncs 0.19;\nmodule lib.fact_restart;\nfn pass(value: u64) -> (result: u64) { return value; }\n";
    let resolver = MapResolver::default().with("lib.fact_restart", library_source);
    let compiler = ReferenceCompiler::default();
    let request = |prior_facts, prior_declaration_facts| VerificationFactRequest {
        language_profile: "0.19".to_owned(),
        subject_language_profiles: BTreeMap::new(),
        prior_facts,
        prior_declaration_facts,
        source_location_identities: BTreeMap::new(),
    };
    let source = "mncs 0.19;\nmodule app.fact_restart;\nuse lib.fact_restart as imported;\nfn entry(value: u64) -> (result: u64) { return imported.pass(value); }\nfn independent(value: u64) -> (result: u64) { return value; }\n";
    let first = compiler.front_end_with_verified_facts(
        resolver.envelope("root", source.to_owned()),
        &resolver,
        &[],
        &request(Vec::new(), Vec::new()),
    );
    assert!(first.is_valid(), "{:?}", first.diagnostics);
    assert_eq!(first.verification_facts.len(), 3);
    assert_eq!(first.declaration_verification_facts.len(), 3);

    // A new process restores the Language-owned facts using only the stable
    // serialized contract; the source snapshot and semantic guards are still
    // re-derived by Language before any validation unit is skipped.
    let serialized_facts =
        serde_json::to_vec(&first.verification_facts).expect("body facts serialize");
    let serialized_declaration_facts = serde_json::to_vec(&first.declaration_verification_facts)
        .expect("declaration facts serialize");
    let restored_facts: Vec<VerificationFact> =
        serde_json::from_slice(&serialized_facts).expect("body facts restore after restart");
    let restored_declaration_facts: Vec<DeclarationVerificationFact> =
        serde_json::from_slice(&serialized_declaration_facts)
            .expect("declaration facts restore after restart");

    let edited_source = format!("// current source snapshot\n{source}");
    let current_envelope = resolver.envelope("root", edited_source.clone());
    let restored = compiler.front_end_with_verified_facts(
        current_envelope.clone(),
        &resolver,
        &[],
        &request(restored_facts.clone(), restored_declaration_facts.clone()),
    );
    assert!(restored.is_valid(), "{:?}", restored.diagnostics);
    let body_reuse = restored
        .verification_fact_reuse
        .as_ref()
        .expect("body fact reuse observation");
    assert_eq!(body_reuse.reused.len(), 3, "{body_reuse:?}");
    assert!(body_reuse.recomputed.is_empty(), "{body_reuse:?}");
    let declaration_reuse = restored
        .declaration_fact_reuse
        .as_ref()
        .expect("declaration fact reuse observation");
    assert_eq!(declaration_reuse.reused.len(), 3, "{declaration_reuse:?}");
    assert!(
        declaration_reuse.recomputed.is_empty(),
        "{declaration_reuse:?}"
    );
    let imported_source_identity = resolver
        .envelope("lib.fact_restart", library_source.to_owned())
        .identity;
    assert!(restored.verification_facts.iter().all(|fact| {
        if fact.subject.as_str().contains("lib.fact_restart") {
            fact.source_location_identity.as_deref() == Some(imported_source_identity.as_str())
        } else {
            fact.source_location_identity.as_deref() == Some(current_envelope.identity.as_str())
        }
    }));

    let mut tampered_facts = restored_facts;
    let entry_fact = tampered_facts
        .iter_mut()
        .find(|fact| fact.subject.as_str().ends_with("app.fact_restart::entry"))
        .expect("entry body fact");
    entry_fact.verification_witness_identity = "0".repeat(64);
    let mut tampered_declaration_facts = restored_declaration_facts;
    let entry_declaration = tampered_declaration_facts
        .iter_mut()
        .find(|fact| fact.subject.as_str().ends_with("app.fact_restart::entry"))
        .expect("entry declaration fact");
    entry_declaration.verification_witness_identity = "0".repeat(64);

    let rechecked = compiler.front_end_with_verified_facts(
        current_envelope,
        &resolver,
        &[],
        &request(tampered_facts, tampered_declaration_facts),
    );
    assert!(rechecked.is_valid(), "{:?}", rechecked.diagnostics);
    let body_reuse = rechecked
        .verification_fact_reuse
        .as_ref()
        .expect("tampered body observation");
    assert_eq!(body_reuse.reused.len(), 2, "{body_reuse:?}");
    assert_eq!(body_reuse.recomputed.len(), 1, "{body_reuse:?}");
    assert!(body_reuse.recomputed[0]
        .subject
        .as_str()
        .ends_with("app.fact_restart::entry"));
    assert_eq!(
        body_reuse.recomputed[0].reason,
        VerificationFactReuseMiss::InvalidWitness
    );
    let declaration_reuse = rechecked
        .declaration_fact_reuse
        .as_ref()
        .expect("tampered declaration observation");
    assert_eq!(declaration_reuse.reused.len(), 2, "{declaration_reuse:?}");
    assert_eq!(
        declaration_reuse.recomputed.len(),
        1,
        "{declaration_reuse:?}"
    );
    assert!(declaration_reuse.recomputed[0]
        .subject
        .as_str()
        .ends_with("app.fact_restart::entry"));
    assert_eq!(
        declaration_reuse.recomputed[0].reason,
        DeclarationFactReuseMiss::InvalidWitness
    );
}

#[test]
fn generic_body_edit_rechecks_specialization_consumers_and_reuses_independent_bodies() {
    let generic_source = r#"mncs 0.10;
module lib.generic;
fn first<N: Nat>(values: [u64; N]) -> (result: u64) { return values[0]; }
fn independent(value: u64) -> (result: u64) { return value; }
"#;
    let resolver = MapResolver::default().with("lib.generic", generic_source);
    let compiler = ReferenceCompiler::default();
    let request = |prior_facts, prior_declaration_facts| VerificationFactRequest {
        language_profile: "0.10".to_owned(),
        subject_language_profiles: BTreeMap::new(),
        prior_facts,
        prior_declaration_facts,
        source_location_identities: BTreeMap::new(),
    };
    let root_source = r#"mncs 0.10;
module app.generic_facts;
use lib.generic as g;
fn entry(values: [u64; 2]) -> (result: u64) { return g.first<2>(values); }
fn other(value: u64) -> (result: u64) { return g.independent(value); }
"#;
    let first_envelope = resolver.envelope("root", root_source.to_owned());
    let first = compiler.front_end_with_verified_facts(
        first_envelope,
        &resolver,
        &[],
        &request(Vec::new(), Vec::new()),
    );
    assert!(first.is_valid(), "{:?}", first.diagnostics);
    let entry_fact = first
        .verification_facts
        .iter()
        .find(|fact| fact.subject.as_str().ends_with("::entry"))
        .expect("generic caller fact");
    assert!(entry_fact.dependencies.iter().any(|dependency| {
        dependency.role == VerificationDependencyRole::GenericSpecialization
            && dependency.identity.as_str().contains("lib.generic")
    }));

    let changed_generic_source = generic_source.replace("values[0]", "values[1]");
    let changed_resolver = MapResolver::default().with("lib.generic", &changed_generic_source);
    let changed_envelope = changed_resolver.envelope("root", root_source.to_owned());
    let changed = compiler.front_end_with_verified_facts(
        changed_envelope,
        &changed_resolver,
        &[],
        &request(
            first.verification_facts,
            first.declaration_verification_facts,
        ),
    );
    assert!(changed.is_valid(), "{:?}", changed.diagnostics);
    let reuse = changed
        .verification_fact_reuse
        .as_ref()
        .expect("Language fact reuse observation");
    assert!(reuse
        .recomputed
        .iter()
        .any(|row| row.subject.as_str().ends_with("::entry")));
    assert!(reuse
        .recomputed
        .iter()
        .any(|row| { row.subject.as_str().contains("lib.generic::first__spec_") }));
    assert!(reuse
        .reused
        .iter()
        .any(|subject| subject.as_str().ends_with("::other")));
}

#[test]
fn generic_argument_edit_checks_new_specialization_and_reuses_unaffected_specialization() {
    let generic_source = r#"mncs 0.10;
module lib.generic_arguments;
fn first<N: Nat>(values: [u64; N]) -> (result: u64) { return values[0]; }
fn independent(value: u64) -> (result: u64) { return value; }
"#;
    let resolver = MapResolver::default().with("lib.generic_arguments", generic_source);
    let compiler = ReferenceCompiler::default();
    let request = |prior_facts, prior_declaration_facts| VerificationFactRequest {
        language_profile: "0.10".to_owned(),
        subject_language_profiles: BTreeMap::new(),
        prior_facts,
        prior_declaration_facts,
        source_location_identities: BTreeMap::new(),
    };
    let root_source = r#"mncs 0.10;
module app.generic_arguments;
use lib.generic_arguments as g;
fn entry_two(values: [u64; 2]) -> (result: u64) { return g.first<2>(values); }
fn entry_three(values: [u64; 3]) -> (result: u64) { return g.first<3>(values); }
fn other(value: u64) -> (result: u64) { return g.independent(value); }
"#;
    let first = compiler.front_end_with_verified_facts(
        resolver.envelope("root", root_source.to_owned()),
        &resolver,
        &[],
        &request(Vec::new(), Vec::new()),
    );
    assert!(first.is_valid(), "{:?}", first.diagnostics);

    let entry_two = first
        .verification_facts
        .iter()
        .find(|fact| fact.subject.as_str().ends_with("::entry_two"))
        .expect("first specialization caller fact");
    let specialization_two = entry_two
        .dependencies
        .iter()
        .find(|dependency| dependency.role == VerificationDependencyRole::GenericSpecialization)
        .expect("first caller's concrete specialization dependency")
        .identity
        .clone();
    assert!(specialization_two
        .as_str()
        .contains("lib.generic_arguments"));
    let specialization_two_record = first
        .program
        .as_ref()
        .expect("linked program")
        .generic_specializations
        .iter()
        .find(|specialization| {
            mncs_model::specialization_id(
                &specialization.generic_function,
                &specialization.canonical_args,
            ) == specialization_two
        })
        .expect("N=2 specialization record");
    let specialization_two_name = specialization_two_record
        .specialization_function
        .as_str()
        .rsplit("::")
        .next()
        .expect("specialization function name");
    let specialization_two_body_subject = first
        .verification_facts
        .iter()
        .find(|fact| {
            fact.subject
                .as_str()
                .ends_with(&format!("::{specialization_two_name}"))
        })
        .expect("N=2 specialization body fact")
        .subject
        .clone();

    // The changed caller gets a new concrete specialization, while entry_two
    // continues to depend on the previously verified N=2 specialization.
    let changed_root_source = root_source.replace(
        "fn entry_three(values: [u64; 3]) -> (result: u64) { return g.first<3>(values); }",
        "fn entry_three(values: [u64; 4]) -> (result: u64) { return g.first<4>(values); }",
    );
    assert_ne!(changed_root_source, root_source);
    let changed = compiler.front_end_with_verified_facts(
        resolver.envelope("root", changed_root_source),
        &resolver,
        &[],
        &request(
            first.verification_facts,
            first.declaration_verification_facts,
        ),
    );
    assert!(changed.is_valid(), "{:?}", changed.diagnostics);

    let reuse = changed
        .verification_fact_reuse
        .as_ref()
        .expect("Language specialization fact observation");
    let changed_caller = reuse
        .recomputed
        .iter()
        .find(|row| {
            row.subject
                .as_str()
                .ends_with("app.generic_arguments::entry_three")
        })
        .expect("caller that selected N=4 must be reverified");
    assert_eq!(
        changed_caller.reason,
        VerificationFactReuseMiss::SubjectChanged
    );
    let new_specialization = reuse
        .recomputed
        .iter()
        .find(|row| {
            row.subject
                .as_str()
                .contains("lib.generic_arguments::first__spec_")
        })
        .expect("new N=4 specialization must have a fresh verification fact");
    assert_eq!(
        new_specialization.reason,
        VerificationFactReuseMiss::Missing
    );
    for independent in [
        "app.generic_arguments::entry_two",
        "app.generic_arguments::other",
        "lib.generic_arguments::independent",
    ] {
        assert!(
            reuse
                .reused
                .iter()
                .any(|subject| subject.as_str().ends_with(independent)),
            "unaffected fact {independent} should remain reusable: {reuse:?}"
        );
    }
    let changed_entry_two = changed
        .verification_facts
        .iter()
        .find(|fact| {
            fact.subject
                .as_str()
                .ends_with("app.generic_arguments::entry_two")
        })
        .expect("unchanged N=2 caller fact remains current");
    assert!(changed_entry_two.dependencies.iter().any(|dependency| {
        dependency.role == VerificationDependencyRole::GenericSpecialization
            && dependency.identity == specialization_two
    }));
    let reused_specialization_bodies = reuse
        .reused
        .iter()
        .filter(|subject| {
            subject
                .as_str()
                .contains("lib.generic_arguments::first__spec_")
        })
        .count();
    assert_eq!(
        reused_specialization_bodies, 1,
        "the unchanged N=2 specialization body should be reused: {reuse:?}"
    );
    assert!(reuse.reused.contains(&specialization_two_body_subject));
    assert!(changed.verification_facts.iter().any(|fact| {
        fact.subject
            .as_str()
            .contains("lib.generic_arguments::first__spec_")
    }));
}

#[test]
fn generic_argument_edit_checks_transitive_specializations_and_reuses_unchanged_closure() {
    let generic_source = r#"mncs 0.10;
module lib.transitive_generic;
fn leaf<N: Nat>(values: [u64; N]) -> (result: u64) { return values[0]; }
fn relay<N: Nat>(values: [u64; N]) -> (result: u64) { return leaf<N>(values); }
fn independent(value: u64) -> (result: u64) { return value; }
"#;
    let resolver = MapResolver::default().with("lib.transitive_generic", generic_source);
    let compiler = ReferenceCompiler::default();
    let request = |prior_facts, prior_declaration_facts| VerificationFactRequest {
        language_profile: "0.10".to_owned(),
        subject_language_profiles: BTreeMap::new(),
        prior_facts,
        prior_declaration_facts,
        source_location_identities: BTreeMap::new(),
    };
    let root_source = r#"mncs 0.10;
module app.transitive_generic;
use lib.transitive_generic as g;
fn entry_two(values: [u64; 2]) -> (result: u64) { return g.relay<2>(values); }
fn entry_three(values: [u64; 3]) -> (result: u64) { return g.relay<3>(values); }
fn other(value: u64) -> (result: u64) { return g.independent(value); }
"#;
    let first = compiler.front_end_with_verified_facts(
        resolver.envelope("root", root_source.to_owned()),
        &resolver,
        &[],
        &request(Vec::new(), Vec::new()),
    );
    assert!(first.is_valid(), "{:?}", first.diagnostics);

    let entry_two = first
        .verification_facts
        .iter()
        .find(|fact| {
            fact.subject
                .as_str()
                .ends_with("app.transitive_generic::entry_two")
        })
        .expect("N=2 caller fact");
    let two_specializations = entry_two
        .dependencies
        .iter()
        .filter(|dependency| dependency.role == VerificationDependencyRole::GenericSpecialization)
        .map(|dependency| dependency.identity.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        two_specializations.len(),
        2,
        "the caller fact must include both relay<2> and its transitive leaf<2> specialization"
    );
    let two_specialization_names = first
        .program
        .as_ref()
        .expect("linked generic program")
        .generic_specializations
        .iter()
        .filter(|specialization| {
            two_specializations.contains(&mncs_model::specialization_id(
                &specialization.generic_function,
                &specialization.canonical_args,
            ))
        })
        .map(|specialization| {
            specialization
                .specialization_function
                .as_str()
                .rsplit("::")
                .next()
                .expect("specialization function name")
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(two_specialization_names.len(), 2);
    let two_specialization_bodies = first
        .verification_facts
        .iter()
        .filter(|fact| {
            two_specialization_names
                .iter()
                .any(|name| fact.subject.as_str().ends_with(&format!("::{name}")))
        })
        .map(|fact| fact.subject.clone())
        .collect::<Vec<_>>();
    assert_eq!(two_specialization_bodies.len(), 2);

    let changed_root_source = root_source.replace(
        "fn entry_three(values: [u64; 3]) -> (result: u64) { return g.relay<3>(values); }",
        "fn entry_three(values: [u64; 4]) -> (result: u64) { return g.relay<4>(values); }",
    );
    assert_ne!(changed_root_source, root_source);
    let changed = compiler.front_end_with_verified_facts(
        resolver.envelope("root", changed_root_source),
        &resolver,
        &[],
        &request(
            first.verification_facts,
            first.declaration_verification_facts,
        ),
    );
    assert!(changed.is_valid(), "{:?}", changed.diagnostics);
    let reuse = changed
        .verification_fact_reuse
        .as_ref()
        .expect("transitive specialization reuse observation");
    assert!(reuse.recomputed.iter().any(|row| {
        row.subject
            .as_str()
            .ends_with("app.transitive_generic::entry_three")
            && row.reason == VerificationFactReuseMiss::SubjectChanged
    }));
    for subject in [
        "lib.transitive_generic::relay__spec_",
        "lib.transitive_generic::leaf__spec_",
    ] {
        assert!(
            reuse.recomputed.iter().any(|row| {
                row.subject.as_str().contains(subject)
                    && row.reason == VerificationFactReuseMiss::Missing
            }),
            "new transitive specialization {subject} must be verified: {reuse:?}"
        );
    }
    assert!(reuse.reused.iter().any(|subject| subject
        .as_str()
        .ends_with("app.transitive_generic::entry_two")));
    assert!(reuse
        .reused
        .iter()
        .any(|subject| subject.as_str().ends_with("app.transitive_generic::other")));
    for subject in two_specialization_bodies {
        assert!(
            reuse.reused.contains(&subject),
            "unchanged N=2 specialization should remain reusable: {reuse:?}"
        );
    }
}

#[test]
fn linked_frontend_invalidates_transitive_callers_on_imported_dependency_edits() {
    let library_source = r#"mncs 0.10;
module lib.fact_dependency;
fn leaf(value: u64) -> (result: u64) { return value; }
fn advance(value: u64) -> (result: u64) { return leaf(value); }
fn independent(value: u64) -> (result: u64) { return value; }
"#;
    let root_source = r#"mncs 0.10;
module app.fact_dependency;
use lib.fact_dependency as dependency;
fn entry(value: u64) -> (result: u64) { return dependency.advance(value); }
fn unrelated(value: u64) -> (result: u64) { return dependency.independent(value); }
fn local(value: u64) -> (result: u64) { return value; }
"#;
    let resolver = MapResolver::default().with("lib.fact_dependency", library_source);
    let compiler = ReferenceCompiler::default();
    let request = |prior_facts, prior_declaration_facts| VerificationFactRequest {
        language_profile: "0.10".to_owned(),
        subject_language_profiles: BTreeMap::new(),
        prior_facts,
        prior_declaration_facts,
        source_location_identities: BTreeMap::new(),
    };

    let first_envelope = resolver.envelope("root", root_source.to_owned());
    let first = compiler.front_end_with_verified_facts(
        first_envelope,
        &resolver,
        &[],
        &request(Vec::new(), Vec::new()),
    );
    assert!(first.is_valid(), "{:?}", first.diagnostics);
    let caller_fact = first
        .verification_facts
        .iter()
        .find(|fact| {
            fact.subject
                .as_str()
                .ends_with("app.fact_dependency::entry")
        })
        .expect("imported callee caller fact");
    assert!(caller_fact.dependencies.iter().any(|dependency| {
        dependency.role == VerificationDependencyRole::CallableImplementation
            && dependency
                .identity
                .as_str()
                .ends_with("lib.fact_dependency::advance")
    }));
    assert!(caller_fact.dependencies.iter().any(|dependency| {
        dependency.role == VerificationDependencyRole::CallableImplementation
            && dependency
                .identity
                .as_str()
                .ends_with("lib.fact_dependency::leaf")
    }));

    let changed_library = library_source.replace(
        "fn leaf(value: u64) -> (result: u64) { return value; }",
        "fn leaf(value: u64) -> (result: u64) { return 0; }",
    );
    assert_ne!(changed_library, library_source);
    let changed_resolver = MapResolver::default().with("lib.fact_dependency", &changed_library);
    let changed = compiler.front_end_with_verified_facts(
        changed_resolver.envelope("root", root_source.to_owned()),
        &changed_resolver,
        &[],
        &request(
            first.verification_facts,
            first.declaration_verification_facts,
        ),
    );
    assert!(changed.is_valid(), "{:?}", changed.diagnostics);

    let reuse = changed
        .verification_fact_reuse
        .as_ref()
        .expect("Language fact reuse observation");
    assert!(reuse
        .recomputed
        .iter()
        .any(|row| row.subject.as_str().ends_with("lib.fact_dependency::leaf")));
    assert!(reuse.recomputed.iter().any(|row| row
        .subject
        .as_str()
        .ends_with("lib.fact_dependency::advance")));
    assert!(reuse
        .recomputed
        .iter()
        .any(|row| row.subject.as_str().ends_with("app.fact_dependency::entry")));
    assert!(reuse.reused.iter().any(|subject| subject
        .as_str()
        .ends_with("lib.fact_dependency::independent")));
    assert!(reuse
        .reused
        .iter()
        .any(|subject| subject.as_str().ends_with("app.fact_dependency::unrelated")));
    assert!(reuse
        .reused
        .iter()
        .any(|subject| subject.as_str().ends_with("app.fact_dependency::local")));

    let declaration_reuse = changed
        .declaration_fact_reuse
        .as_ref()
        .expect("Language declaration fact reuse observation");
    assert_eq!(
        declaration_reuse.reused.len(),
        changed.declaration_verification_facts.len()
    );
    assert!(declaration_reuse.recomputed.is_empty());
}

#[test]
fn linked_frontend_invalidates_imported_signature_consumers_and_reuses_independent_facts() {
    let library_source = r#"mncs 0.10;
module lib.interface_facts;
fn advance(value: u64) -> (result: u64) { return value; }
fn independent(value: u64) -> (result: u64) { return value; }
"#;
    let root_source = r#"mncs 0.10;
module app.interface_facts;
use lib.interface_facts as dependency;
fn entry(value: u64) -> (result: u64) { return dependency.advance(value); }
fn unrelated(value: u64) -> (result: u64) { return dependency.independent(value); }
fn local(value: u64) -> (result: u64) { return value; }
"#;
    let resolver = MapResolver::default().with("lib.interface_facts", library_source);
    let compiler = ReferenceCompiler::default();
    let request = |prior_facts, prior_declaration_facts| VerificationFactRequest {
        language_profile: "0.10".to_owned(),
        subject_language_profiles: BTreeMap::new(),
        prior_facts,
        prior_declaration_facts,
        source_location_identities: BTreeMap::new(),
    };

    let first = compiler.front_end_with_verified_facts(
        resolver.envelope("root", root_source.to_owned()),
        &resolver,
        &[],
        &request(Vec::new(), Vec::new()),
    );
    assert!(first.is_valid(), "{:?}", first.diagnostics);
    assert_eq!(first.verification_facts.len(), 5);

    // Renaming an imported output is a signature change even though the
    // positional call remains type-correct. The Language frontend must
    // recheck that consumer while retaining facts outside its dependency edge.
    let changed_library = library_source.replace(
        "fn advance(value: u64) -> (result: u64)",
        "fn advance(value: u64) -> (updated: u64)",
    );
    assert_ne!(changed_library, library_source);
    let changed_resolver = MapResolver::default().with("lib.interface_facts", &changed_library);
    let changed = compiler.front_end_with_verified_facts(
        changed_resolver.envelope("root", root_source.to_owned()),
        &changed_resolver,
        &[],
        &request(
            first.verification_facts,
            first.declaration_verification_facts,
        ),
    );
    assert!(changed.is_valid(), "{:?}", changed.diagnostics);
    assert!(changed.diagnostics.is_empty());

    let reuse = changed
        .verification_fact_reuse
        .as_ref()
        .expect("Language function-body fact observation");
    let advance_recomputation = reuse
        .recomputed
        .iter()
        .find(|row| {
            row.subject
                .as_str()
                .ends_with("lib.interface_facts::advance")
        })
        .expect("changed imported function rechecked against its new interface");
    assert_eq!(
        advance_recomputation.reason,
        VerificationFactReuseMiss::DeclarationInterfaceChanged
    );
    let entry_recomputation = reuse
        .recomputed
        .iter()
        .find(|row| row.subject.as_str().ends_with("app.interface_facts::entry"))
        .expect("unchanged caller invalidated by imported signature change");
    assert_eq!(
        entry_recomputation.reason,
        VerificationFactReuseMiss::DependenciesChanged
    );
    assert_eq!(reuse.recomputed.len(), 2, "{reuse:?}");
    assert_eq!(reuse.reused.len(), 3, "{reuse:?}");
    for independent in [
        "lib.interface_facts::independent",
        "app.interface_facts::unrelated",
        "app.interface_facts::local",
    ] {
        assert!(
            reuse
                .reused
                .iter()
                .any(|subject| subject.as_str().ends_with(independent)),
            "independent fact {independent} should remain reusable: {reuse:?}"
        );
    }
    assert!(changed.verification_facts.iter().any(|fact| {
        fact.subject
            .as_str()
            .ends_with("app.interface_facts::entry")
    }));

    let declarations = changed
        .declaration_fact_reuse
        .as_ref()
        .expect("Language declaration fact observation");
    assert!(declarations.recomputed.iter().any(|row| {
        row.subject
            .as_str()
            .ends_with("lib.interface_facts::advance")
    }));
    for independent in [
        "lib.interface_facts::independent",
        "app.interface_facts::unrelated",
        "app.interface_facts::local",
    ] {
        assert!(
            declarations
                .reused
                .iter()
                .any(|subject| subject.as_str().ends_with(independent)),
            "independent declaration fact {independent} should remain reusable: {declarations:?}"
        );
    }
}

#[test]
fn linked_frontend_invalidates_imported_nominal_type_consumers_and_keeps_diagnostics_fresh() {
    let library_source = r#"mncs 0.10;
module lib.nominal_facts;
record State { value: u64 }
fn read(state: State) -> (result: u64) { return state.value; }
fn independent(value: u64) -> (result: u64) { return value; }
"#;
    let root_source = r#"mncs 0.10;
module app.nominal_facts;
use lib.nominal_facts as dependency;
fn entry(state: dependency.State) -> (result: u64) { return dependency.read(state); }
fn unrelated(value: u64) -> (result: u64) { return dependency.independent(value); }
fn local(value: u64) -> (result: u64) { return value; }
"#;
    let resolver = MapResolver::default().with("lib.nominal_facts", library_source);
    let compiler = ReferenceCompiler::default();
    let request = |prior_facts, prior_declaration_facts| VerificationFactRequest {
        language_profile: "0.10".to_owned(),
        subject_language_profiles: BTreeMap::new(),
        prior_facts,
        prior_declaration_facts,
        source_location_identities: BTreeMap::new(),
    };

    let first = compiler.front_end_with_verified_facts(
        resolver.envelope("root", root_source.to_owned()),
        &resolver,
        &[],
        &request(Vec::new(), Vec::new()),
    );
    assert!(first.is_valid(), "{:?}", first.diagnostics);
    assert_eq!(first.verification_facts.len(), 5);

    // Changing a field name changes the nominal identity carried by both the
    // imported callee and its consumer, even though their source call sites
    // remain textually unchanged.
    let changed_library = library_source
        .replace(
            "record State { value: u64 }",
            "record State { amount: u64 }",
        )
        .replace("state.value", "state.amount");
    assert_ne!(changed_library, library_source);
    let changed_resolver = MapResolver::default().with("lib.nominal_facts", &changed_library);
    let changed = compiler.front_end_with_verified_facts(
        changed_resolver.envelope("root", root_source.to_owned()),
        &changed_resolver,
        &[],
        &request(
            first.verification_facts,
            first.declaration_verification_facts,
        ),
    );
    assert!(changed.is_valid(), "{:?}", changed.diagnostics);
    let reuse = changed
        .verification_fact_reuse
        .as_ref()
        .expect("Language function-body fact observation");
    let read_recomputation = reuse
        .recomputed
        .iter()
        .find(|row| row.subject.as_str().ends_with("lib.nominal_facts::read"))
        .expect("changed imported projection rechecked");
    assert_eq!(
        read_recomputation.reason,
        VerificationFactReuseMiss::SubjectChanged
    );
    let entry_recomputation = reuse
        .recomputed
        .iter()
        .find(|row| row.subject.as_str().ends_with("app.nominal_facts::entry"))
        .expect("consumer of changed nominal interface rechecked");
    assert_eq!(
        entry_recomputation.reason,
        VerificationFactReuseMiss::SubjectChanged
    );
    assert_eq!(reuse.recomputed.len(), 2, "{reuse:?}");
    assert_eq!(reuse.reused.len(), 3, "{reuse:?}");
    for independent in [
        "lib.nominal_facts::independent",
        "app.nominal_facts::unrelated",
        "app.nominal_facts::local",
    ] {
        assert!(
            reuse
                .reused
                .iter()
                .any(|subject| subject.as_str().ends_with(independent)),
            "unrelated fact {independent} should remain reusable: {reuse:?}"
        );
    }

    // A cached success from the changed-but-valid snapshot cannot hide a new
    // error when the record shape changes but its old projection remains.
    let invalid_library = changed_library.replace("state.amount", "state.value");
    let invalid_resolver = MapResolver::default().with("lib.nominal_facts", &invalid_library);
    let invalid = compiler.front_end_with_verified_facts(
        invalid_resolver.envelope("root", root_source.to_owned()),
        &invalid_resolver,
        &[],
        &request(
            changed.verification_facts,
            changed.declaration_verification_facts,
        ),
    );
    assert!(!invalid.is_valid());
    assert!(
        invalid.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "MNE172"
                && diagnostic
                    .related
                    .iter()
                    .any(|related| related.code == "MNE162")
        }),
        "fresh unknown-field diagnostic must survive import wrapping: {:?}",
        invalid.diagnostics
    );
}

#[test]
fn linked_frontend_invalidates_imported_effect_and_capability_dependencies() {
    let effect_module = r#"mncs 0.10;
module lib.effect_facts;
fn access(value: u64) -> (result: u64)
    capability storage_read
    effect read_storage authorized_by storage_read
{
    return value;
}
fn independent(value: u64) -> (result: u64) { return value; }
"#;
    let effect_root = r#"mncs 0.10;
module app.effect_facts;
use lib.effect_facts as imported;
fn entry(value: u64) -> (result: u64)
    capability storage_read
    effect read_storage authorized_by storage_read
    effect write_storage authorized_by storage_read
{
    return imported.access(value);
}
fn unrelated(value: u64) -> (result: u64) { return value; }
"#;
    let compiler = ReferenceCompiler::default();
    let request = |prior_facts, prior_declaration_facts| VerificationFactRequest {
        language_profile: "0.10".to_owned(),
        subject_language_profiles: BTreeMap::new(),
        prior_facts,
        prior_declaration_facts,
        source_location_identities: BTreeMap::new(),
    };
    let first_resolver = MapResolver::default().with("lib.effect_facts", effect_module);
    let first = compiler.front_end_with_verified_facts(
        first_resolver.envelope("root", effect_root.to_owned()),
        &first_resolver,
        &[],
        &request(Vec::new(), Vec::new()),
    );
    assert!(first.is_valid(), "{:?}", first.diagnostics);
    assert_eq!(first.verification_facts.len(), 4);

    let first_entry = first
        .verification_facts
        .iter()
        .find(|fact| fact.subject.as_str().ends_with("app.effect_facts::entry"))
        .expect("imported effect caller fact");
    let first_effect_dependencies = first_entry
        .dependencies
        .iter()
        .filter(|dependency| dependency.role == VerificationDependencyRole::Effect)
        .map(|dependency| (dependency.identity.clone(), dependency.fingerprint.clone()))
        .collect::<Vec<_>>();
    assert!(!first_effect_dependencies.is_empty());

    // The caller already declares both effects, so changing the imported
    // function's effect keeps the program valid while changing a guarded call
    // assumption. In the resolved function body that call summary also affects
    // the caller's semantic subject fingerprint, which is why Language reports
    // SubjectChanged for the exact caller.
    let changed_effect_module =
        effect_module.replace("effect read_storage", "effect write_storage");
    assert_ne!(changed_effect_module, effect_module);
    let changed_effect_resolver =
        MapResolver::default().with("lib.effect_facts", &changed_effect_module);
    let changed_effect = compiler.front_end_with_verified_facts(
        changed_effect_resolver.envelope("root", effect_root.to_owned()),
        &changed_effect_resolver,
        &[],
        &request(
            first.verification_facts.clone(),
            first.declaration_verification_facts.clone(),
        ),
    );
    assert!(
        changed_effect.is_valid(),
        "{:?}",
        changed_effect.diagnostics
    );
    let effect_reuse = changed_effect
        .verification_fact_reuse
        .as_ref()
        .expect("effect dependency reuse observation");
    let effect_entry = effect_reuse
        .recomputed
        .iter()
        .find(|row| row.subject.as_str().ends_with("app.effect_facts::entry"))
        .expect("imported effect change invalidates its caller");
    assert_eq!(
        effect_entry.reason,
        VerificationFactReuseMiss::SubjectChanged
    );
    assert_eq!(effect_reuse.recomputed.len(), 2, "{effect_reuse:?}");
    assert!(effect_reuse
        .recomputed
        .iter()
        .any(|row| row.subject.as_str().ends_with("lib.effect_facts::access")));
    let effect_access = effect_reuse
        .recomputed
        .iter()
        .find(|row| row.subject.as_str().ends_with("lib.effect_facts::access"))
        .expect("changed imported effect body rechecked");
    assert_eq!(
        effect_access.reason,
        VerificationFactReuseMiss::EffectAssumptionsChanged
    );
    assert_eq!(effect_reuse.reused.len(), 2, "{effect_reuse:?}");
    assert!(effect_reuse
        .reused
        .iter()
        .any(|subject| subject.as_str().ends_with("lib.effect_facts::independent")));
    assert!(effect_reuse
        .reused
        .iter()
        .any(|subject| subject.as_str().ends_with("app.effect_facts::unrelated")));
    let changed_entry = changed_effect
        .verification_facts
        .iter()
        .find(|fact| fact.subject.as_str().ends_with("app.effect_facts::entry"))
        .expect("reverified imported effect caller fact");
    let changed_effect_dependencies = changed_entry
        .dependencies
        .iter()
        .filter(|dependency| dependency.role == VerificationDependencyRole::Effect)
        .map(|dependency| (dependency.identity.clone(), dependency.fingerprint.clone()))
        .collect::<Vec<_>>();
    assert_ne!(first_effect_dependencies, changed_effect_dependencies);
    assert_ne!(
        first_entry.subject_fingerprint,
        changed_entry.subject_fingerprint
    );
    assert!(effect_reuse
        .reused
        .iter()
        .any(|subject| subject.as_str().ends_with("app.effect_facts::unrelated")));

    let capability_module = r#"mncs 0.10;
module lib.capability_facts;
fn access(value: u64) -> (result: u64)
    capability storage_read
{
    return value;
}
fn independent(value: u64) -> (result: u64) { return value; }
"#;
    let capability_root = r#"mncs 0.10;
module app.capability_facts;
use lib.capability_facts as imported;
fn entry(value: u64) -> (result: u64)
    capability storage_read
    capability storage_write
{
    return imported.access(value);
}
fn unrelated(value: u64) -> (result: u64) { return value; }
"#;
    let first_capability_resolver =
        MapResolver::default().with("lib.capability_facts", capability_module);
    let first_capability = compiler.front_end_with_verified_facts(
        first_capability_resolver.envelope("root", capability_root.to_owned()),
        &first_capability_resolver,
        &[],
        &request(Vec::new(), Vec::new()),
    );
    assert!(
        first_capability.is_valid(),
        "{:?}",
        first_capability.diagnostics
    );
    assert_eq!(first_capability.verification_facts.len(), 4);

    let first_capability_entry = first_capability
        .verification_facts
        .iter()
        .find(|fact| {
            fact.subject
                .as_str()
                .ends_with("app.capability_facts::entry")
        })
        .expect("imported capability caller fact");
    let first_capability_subject_fingerprint = first_capability_entry.subject_fingerprint.clone();
    let first_capability_dependencies = first_capability_entry
        .dependencies
        .iter()
        .filter(|dependency| dependency.role == VerificationDependencyRole::Capability)
        .map(|dependency| (dependency.identity.clone(), dependency.fingerprint.clone()))
        .collect::<Vec<_>>();
    assert!(!first_capability_dependencies.is_empty());

    // Both capabilities are already admitted by the caller. The changed
    // imported call requirement remains valid but changes the caller's
    // resolved semantic body and guarded capability dependency.
    let changed_capability_module =
        capability_module.replace("capability storage_read", "capability storage_write");
    assert_ne!(changed_capability_module, capability_module);
    let changed_capability_resolver =
        MapResolver::default().with("lib.capability_facts", &changed_capability_module);
    let changed_capability = compiler.front_end_with_verified_facts(
        changed_capability_resolver.envelope("root", capability_root.to_owned()),
        &changed_capability_resolver,
        &[],
        &request(
            first_capability.verification_facts,
            first_capability.declaration_verification_facts,
        ),
    );
    assert!(
        changed_capability.is_valid(),
        "{:?}",
        changed_capability.diagnostics
    );
    let capability_reuse = changed_capability
        .verification_fact_reuse
        .as_ref()
        .expect("capability dependency reuse observation");
    let capability_entry = capability_reuse
        .recomputed
        .iter()
        .find(|row| {
            row.subject
                .as_str()
                .ends_with("app.capability_facts::entry")
        })
        .expect("imported capability change invalidates its caller");
    assert_eq!(
        capability_entry.reason,
        VerificationFactReuseMiss::SubjectChanged
    );
    assert_eq!(capability_reuse.recomputed.len(), 2, "{capability_reuse:?}");
    assert!(capability_reuse.recomputed.iter().any(|row| {
        row.subject
            .as_str()
            .ends_with("lib.capability_facts::access")
    }));
    let capability_access = capability_reuse
        .recomputed
        .iter()
        .find(|row| {
            row.subject
                .as_str()
                .ends_with("lib.capability_facts::access")
        })
        .expect("changed imported capability body rechecked");
    assert_eq!(
        capability_access.reason,
        VerificationFactReuseMiss::CapabilityAssumptionsChanged
    );
    assert_eq!(capability_reuse.reused.len(), 2, "{capability_reuse:?}");
    assert!(capability_reuse.reused.iter().any(|subject| {
        subject
            .as_str()
            .ends_with("lib.capability_facts::independent")
    }));
    assert!(capability_reuse.reused.iter().any(|subject| {
        subject
            .as_str()
            .ends_with("app.capability_facts::unrelated")
    }));
    let changed_capability_entry = changed_capability
        .verification_facts
        .iter()
        .find(|fact| {
            fact.subject
                .as_str()
                .ends_with("app.capability_facts::entry")
        })
        .expect("reverified imported capability caller fact");
    let changed_capability_dependencies = changed_capability_entry
        .dependencies
        .iter()
        .filter(|dependency| dependency.role == VerificationDependencyRole::Capability)
        .map(|dependency| (dependency.identity.clone(), dependency.fingerprint.clone()))
        .collect::<Vec<_>>();
    assert_ne!(
        first_capability_dependencies,
        changed_capability_dependencies
    );
    assert_ne!(
        first_capability_subject_fingerprint,
        changed_capability_entry.subject_fingerprint
    );
    assert!(capability_reuse.reused.iter().any(|subject| subject
        .as_str()
        .ends_with("app.capability_facts::unrelated")));
}
