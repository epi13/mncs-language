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

// Pressure reproducer: in Profile 0.9 `value.field` parses as a two-segment
// path for later reinterpretation. Postfix chaining must apply to that parse
// exactly as for any other primary, so record-held sequences observe
// elements (`rec.items[i]`) and chains continue (`.field`, calls).
#[test]
fn record_projection_chains_support_element_observation() {
    elaborate(
        r#"
mncs 0.9;
module app.chains;
record Inner { v: u64 }
record Outer { items: [Inner; 4], n: u64 }
fn read(o: Outer) -> (result: u64) {
    return o.items[0].v +% o.n;
}
fn call_arg(o: Outer) -> (result: u64) {
    return first_v(o.items[1]);
}
fn first_v(item: Inner) -> (result: u64) {
    return item.v;
}
"#,
    )
    .expect("projection chains elaborate");
}

#[test]
fn finite_constructors_still_resolve_nominally() {
    let program = elaborate(
        r#"
mncs 0.9;
module app.finite;
enum Verdict { Yes, No }
fn decide(hit: bool) -> (result: Verdict) {
    if hit {
        return Verdict.Yes;
    }
    return Verdict.No;
}
"#,
    )
    .expect("finite constructors elaborate");
    assert_eq!(program.functions.len(), 1);
}
