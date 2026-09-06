use mncs_compiler::{elaborate_program_with_resolver, ModuleResolver};
use mncs_syntax::{parse, SourceArtifactKind, SourceEnvelope};

#[derive(Default)]
struct MapResolver {
    modules: std::collections::BTreeMap<String, String>,
}

impl MapResolver {
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

fn elaborate(text: &str) -> Result<mncs_model::Program, Vec<String>> {
    let resolver = MapResolver::default();
    let envelope = resolver.envelope("root", text.to_owned());
    let parsed = parse(&envelope);
    assert!(
        parsed.is_valid(),
        "fixture parses: {:?}",
        parsed.diagnostics
    );
    let ast = parsed.ast.expect("fixture parses");
    match elaborate_program_with_resolver(&ast, &resolver).0 {
        Ok(program) => Ok(program),
        Err(errors) => Err(errors.iter().map(|d| d.code.clone()).collect()),
    }
}

const PREAMBLE: &str = r#"
mncs 0.9;
module app.contracts;
fn add(a: i64, b: i64) -> (result: i64) {
    return a +% b;
}
fn sub(a: i64, b: i64) -> (result: i64) {
    return a -% b;
}
"#;

#[test]
fn executable_contract_bindings_elaborate_with_kinds() {
    let program = elaborate(&format!(
        r#"{PREAMBLE}
fn add_sub_roundtrip(x: i64, y: i64) -> (result: bool) {{
    return sub(add(x, y), y) == x;
}}
fn add_commutes(x: i64, y: i64) -> (result: bool) {{
    return add(x, y) == add(y, x);
}}
fn guarded_add(a: i64, b: i64) -> (result: i64)
    property add_sub_roundtrip
    metamorphic add_commutes
    invariant add_commutes
{{
    return add(a, b);
}}
"#
    ))
    .expect("executable contracts elaborate");
    let guarded = program
        .functions
        .iter()
        .find(|function| function.name == "guarded_add")
        .expect("guarded_add present");
    let kinds: Vec<String> = guarded
        .contracts
        .iter()
        .map(|clause| format!("{:?}", clause.kind))
        .collect();
    assert_eq!(kinds, vec!["Property", "Metamorphic", "Invariant"]);
    assert!(guarded
        .contracts
        .iter()
        .all(|clause| clause.expression == clause.id && !clause.id.is_empty()));
}

#[test]
fn executable_contract_rejects_unknown_predicate() {
    let errors = elaborate(&format!(
        r#"{PREAMBLE}
fn guarded_add(a: i64, b: i64) -> (result: i64)
    property no_such_predicate
{{
    return add(a, b);
}}
"#
    ))
    .expect_err("unknown predicate must fail");
    assert!(errors.contains(&"MNE231".to_owned()), "codes: {errors:?}");
}

#[test]
fn executable_contract_rejects_non_boolean_predicate() {
    let errors = elaborate(&format!(
        r#"{PREAMBLE}
fn guarded_add(a: i64, b: i64) -> (result: i64)
    property add
{{
    return add(a, b);
}}
"#
    ))
    .expect_err("non-bool predicate must fail");
    assert!(errors.contains(&"MNE233".to_owned()), "codes: {errors:?}");
}

#[test]
fn executable_contract_requires_profile_0_9() {
    let resolver = MapResolver::default();
    let text = r#"
mncs 0.8;
module app.legacy;
fn ok_predicate(x: i64) -> (result: bool) {
    return x == x;
}
fn guarded(x: i64) -> (result: i64)
    property ok_predicate
{
    return x;
}
"#;
    let envelope = resolver.envelope("root", text.to_owned());
    let parsed = parse(&envelope);
    assert!(parsed.is_valid(), "fixture parses");
    let ast = parsed.ast.expect("fixture parses");
    let errors = match elaborate_program_with_resolver(&ast, &resolver).0 {
        Ok(_) => panic!("profile 0.8 executable contract must fail"),
        Err(errors) => errors.iter().map(|d| d.code.clone()).collect::<Vec<_>>(),
    };
    assert!(errors.contains(&"MNE230".to_owned()), "codes: {errors:?}");
}

#[test]
fn legacy_contract_names_stay_unchecked() {
    elaborate(&format!(
        r#"{PREAMBLE}
fn guarded_add(a: i64, b: i64) -> (result: i64)
    requires n_within_limit
{{
    return add(a, b);
}}
"#
    ))
    .expect("legacy requires names stay unchecked");
}
