use std::collections::BTreeMap;

use mncs_compiler::{elaborate_program_with_resolver, ModuleResolver};
use mncs_model::BodyType;
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
        Err(errors) => Err(errors
            .iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect()),
    }
}

fn body_type_of(program: &mncs_model::Program, function: &str, value: &str) -> BodyType {
    let function = program
        .functions
        .iter()
        .find(|f| f.name == function)
        .expect("function exists");
    let body = function.body.as_ref().expect("executable body");
    body.parameters
        .iter()
        .find(|p| p.name == value)
        .map(|p| p.ty.clone())
        .or_else(|| {
            body.blocks
                .iter()
                .flat_map(|b| b.operations.iter())
                .flat_map(|op| op.results.iter())
                .find(|r| r.id == value)
                .map(|r| r.ty.clone())
        })
        .expect("value exists")
}

#[test]
fn bool_is_semantic_through_elaboration() {
    let resolver = MapResolver::default();
    let program = elaborate(
        &resolver,
        r#"
mncs 0.10;
module app.bool_sem;
fn id(b: bool) -> (result: bool) { return b; }
"#,
    )
    .expect("bool identity elaborates");
    let report = program.validate();
    assert!(report.valid, "bool program validates: {:?}", report.errors);
    assert!(!report.errors.iter().any(|e| e.code == "MNB122"));
    let ty = body_type_of(&program, "id", "b");
    assert_eq!(ty, BodyType::Bool);
    assert_eq!(ty.semantic_name(), "bool");
}

#[test]
fn nested_sequences_of_imported_nominal_records_keep_identity() {
    let resolver = MapResolver::default().with(
        "lib.geo",
        r#"
mncs 0.10;
module lib.geo;
record Point { x: i64, y: i64 }
"#,
    );
    let program = elaborate(
        &resolver,
        r#"
mncs 0.10;
module app.nested;
use lib.geo;
fn Napaeo(grid: [[lib.geo.Point; 2]; 3]) -> (result: lib.geo.Point) {
    return grid[0][1];
}
"#,
    )
    .expect("nested nominal sequences elaborate");
    let report = program.validate();
    assert!(
        report.valid,
        "nested program validates: {:?}",
        report.errors
    );
    let point = program
        .record_types
        .iter()
        .find(|r| r.name == "Point")
        .expect("linked Point");
    let ty = body_type_of(&program, "Napaeo", "grid");
    match ty {
        BodyType::Sequence {
            element: outer,
            bound,
        } => {
            assert_eq!(bound, mncs_model::SequenceBound::Exact(3));
            match *outer {
                BodyType::Sequence {
                    element: inner,
                    bound,
                } => {
                    assert_eq!(bound, mncs_model::SequenceBound::Exact(2));
                    match *inner {
                        BodyType::Record { identity, name } => {
                            assert_eq!(identity, point.identity);
                            assert_eq!(name, "Point");
                        }
                        other => panic!("inner element is not Point: {other:?}"),
                    }
                }
                other => panic!("outer element is not a sequence: {other:?}"),
            }
        }
        other => panic!("grid is not a nested sequence: {other:?}"),
    }
}

#[test]
fn structurally_identical_records_from_two_modules_stay_distinct() {
    let resolver = MapResolver::default()
        .with(
            "lib.left",
            "mncs 0.10;\nmodule lib.left;\nrecord R { x: i64 }\n",
        )
        .with(
            "lib.right",
            "mncs 0.10;\nmodule lib.right;\nrecord R { x: i64 }\n",
        );
    let program = elaborate(
        &resolver,
        r#"
mncs 0.10;
module app.split;
use lib.left;
use lib.right;
fn make_left() -> (result: lib.left.R) {
    return lib.left.R { x: 1 };
}
fn make_right() -> (result: lib.right.R) {
    return lib.right.R { x: 2 };
}
"#,
    )
    .expect("both records elaborate");
    let report = program.validate();
    assert!(report.valid, "split program validates: {:?}", report.errors);
    let left = program
        .record_types
        .iter()
        .find(|r| r.identity.0.contains("lib.left"))
        .expect("left R");
    let right = program
        .record_types
        .iter()
        .find(|r| r.identity.0.contains("lib.right"))
        .expect("right R");
    assert_ne!(left.identity, right.identity);
    assert_ne!(
        BodyType::Record {
            identity: left.identity.clone(),
            name: left.name.clone()
        }
        .canonical_identity(),
        BodyType::Record {
            identity: right.identity.clone(),
            name: right.name.clone()
        }
        .canonical_identity()
    );
}

#[test]
fn generic_bool_sequence_specializes_without_unresolved_names() {
    let resolver = MapResolver::default();
    let program = elaborate(
        &resolver,
        r#"
mncs 0.10;
module app.genbool;
fn first<T, N: Nat>(xs: [T; N]) -> (result: T) { return xs[0]; }
fn demo() -> (result: bool) {
    let xs: [bool; 2] = [true, false];
    return first<bool, 2>(xs);
}
"#,
    )
    .expect("generic bool call elaborates");
    let report = program.validate();
    assert!(report.valid, "generic bool validates: {:?}", report.errors);
    assert!(!report.errors.iter().any(|e| e.code == "MNB122"));
    let spec = program
        .generic_specializations
        .iter()
        .find(|s| s.canonical_args.contains("bool"))
        .expect("bool specialization recorded");
    assert!(
        spec.canonical_args.contains("bool"),
        "canonical args carry bool identity: {}",
        spec.canonical_args
    );
}

#[test]
fn record_containing_bounded_sequence_round_trips() {
    let resolver = MapResolver::default();
    let program = elaborate(
        &resolver,
        r#"
mncs 0.10;
module app.buf;
record Buf { data: [byte; 8], flag: bool }
fn get(b: Buf) -> (result: byte) { return b.data[0]; }
fn is_set(b: Buf) -> (result: bool) { return b.flag; }
"#,
    )
    .expect("record of sequences elaborates");
    let report = program.validate();
    assert!(
        report.valid,
        "record program validates: {:?}",
        report.errors
    );
    let buf = program
        .record_types
        .iter()
        .find(|r| r.name == "Buf")
        .expect("Buf");
    let data = buf
        .fields
        .iter()
        .find(|f| f.name == "data")
        .expect("data field");
    assert_eq!(data.field_type, "[byte; 8]");
    let flag = buf
        .fields
        .iter()
        .find(|f| f.name == "flag")
        .expect("flag field");
    assert_eq!(flag.field_type, "bool");
}

#[test]
fn byte_and_u8_are_distinct_types() {
    assert_ne!(
        BodyType::from_semantic_name("byte"),
        BodyType::from_semantic_name("u8")
    );
    let resolver = MapResolver::default();
    let errors = elaborate(
        &resolver,
        r#"
mncs 0.10;
module app.mixed;
fn take_byte(b: byte) -> (result: byte) { return b; }
fn demo(x: u8) -> (result: byte) { return take_byte(x); }
"#,
    )
    .err()
    .expect("byte/u8 confusion is rejected");
    assert!(
        errors.iter().any(|e| e.contains("MNE")),
        "rejection carries a coded diagnostic: {errors:?}"
    );
}

#[test]
fn unknown_type_name_is_rejected_not_preserved() {
    let resolver = MapResolver::default();
    let errors = elaborate(
        &resolver,
        r#"
mncs 0.10;
module app.unknown;
fn id(x: Mystery) -> (result: Mystery) { return x; }
"#,
    )
    .err()
    .expect("unknown nominal is rejected");
    assert!(
        errors.iter().any(|e| e.contains("MNE105")),
        "unknown type carries MNE105: {errors:?}"
    );
}
