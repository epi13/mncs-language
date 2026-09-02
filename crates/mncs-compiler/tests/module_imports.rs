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
        (
            "missing",
            "fn id<T>(value: T) -> (result: T) { return value; }\nfn demo() -> (result: i64) { return id(1); }",
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
            "out_of_range",
            "fn width<N: Nat>(xs: [i64; N]) -> (result: i64) { return xs[0]; }\nfn demo() -> (result: i64) { let xs: [i64; 1] = [1]; return width<65>(xs); }",
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
