//! CP-0009 (iteration identities): hygienic recorded identities.
//!
//! Sequential loops may reuse a source-level index name. The recorded
//! `BodyBoundedIteration.id` — the identity the proof graph, obligation
//! subjects, and MNB061 uniqueness all key off — must still be unique per
//! function: the first use keeps the bare source name (stable fingerprints
//! for all existing programs) and later reuses take `name#2`, `name#3`.
//! `#` never lexes inside a source name, so machine identities can never
//! collide with user-written ones.

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

fn iteration_ids(program: &mncs_model::Program, function: &str) -> Vec<String> {
    program
        .functions
        .iter()
        .find(|candidate| candidate.name == function)
        .unwrap_or_else(|| panic!("function {function} elaborated"))
        .body
        .as_ref()
        .expect("function has a body")
        .bounded_iterations
        .iter()
        .map(|iteration| iteration.id.clone())
        .collect()
}

#[test]
fn sequential_traversal_reuse_records_hygienic_identities() {
    let program = elaborate(
        r#"mncs 0.13;
module app.iterreuse;
fn f(a: [u64; 2], b: [u64; 2]) -> (result: u64) {
    iterate i over a carrying x: u64 = 0 {
        next x = x + 1;
    }
    iterate i over b carrying y: u64 = x {
        next y = y + 1;
    }
    return y;
}
"#,
    )
    .expect("sequential reuse elaborates");
    assert_eq!(iteration_ids(&program, "f"), vec!["i", "i#2"]);
    let report = program.validate();
    assert!(
        report.valid,
        "reused identities validate (MNB061): {:?}",
        report.errors
    );
}

#[test]
fn sequential_counted_reuse_counts_per_name() {
    let program = elaborate(
        r#"mncs 0.13;
module app.iterreuse;
fn f() -> (result: u64) {
    iterate t up_to 2 carrying a: u64 = 0 {
        next a = a + 1;
    }
    iterate t up_to 2 carrying b: u64 = a {
        next b = b + 1;
    }
    iterate t up_to 2 carrying c: u64 = b {
        next c = c + 1;
    }
    return c;
}
"#,
    )
    .expect("triple reuse elaborates");
    assert_eq!(iteration_ids(&program, "f"), vec!["t", "t#2", "t#3"]);
    let report = program.validate();
    assert!(
        report.valid,
        "reused identities validate (MNB061): {:?}",
        report.errors
    );
}

#[test]
fn single_loop_keeps_bare_identity() {
    let program = elaborate(
        r#"mncs 0.13;
module app.itersingle;
fn f(a: [u64; 2]) -> (result: u64) {
    iterate solo over a carrying x: u64 = 0 {
        next x = x + 1;
    }
    return x;
}
"#,
    )
    .expect("single loop elaborates");
    // No suffix on first (and only) use: existing fingerprints are stable.
    assert_eq!(iteration_ids(&program, "f"), vec!["solo"]);
}

#[test]
fn nested_overlap_keeps_mne146() {
    let errors = elaborate(
        r#"mncs 0.13;
module app.iternested;
fn f(a: [u64; 2], b: [u64; 2]) -> (result: u64) {
    iterate i over a carrying x: u64 = 0 {
        iterate i over b carrying y: u64 = x {
            next y = y + 1;
        }
        next x = y;
    }
    return x;
}
"#,
    )
    .expect_err("nested overlap stays rejected");
    assert!(
        errors.iter().any(|code| code == "MNE146"),
        "expected MNE146 in {errors:?}"
    );
}
