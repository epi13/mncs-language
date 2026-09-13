use mncs_compiler::ReferenceCompiler;
use mncs_model::{program_id, test_declaration_id};
use mncs_syntax::{SourceArtifactKind, SourceEnvelope};

fn front_end(source: &str) -> mncs_compiler::SourceFrontEndResult {
    ReferenceCompiler::default().front_end(SourceEnvelope::inline(
        SourceArtifactKind::Program,
        "inventory-fixture",
        source,
    ))
}

#[test]
fn compiler_emits_a_deterministic_structural_test_inventory() {
    let output = front_end(
        "mncs 0.17;\nmodule example.inventory;\n\nfn production() -> (result: i64) { return 7; }\ntest second() -> (result: i64) { return 2; }\ntest first() -> (result: i64) { return 1; }\n",
    );
    assert!(output.is_valid(), "{:#?}", output.diagnostics);
    let inventory = output.test_inventory.expect("valid source has inventory");
    assert_eq!(inventory.schema_version, "mncs.test-inventory/1");
    assert_eq!(inventory.subject_identity, program_id("example.inventory"));
    assert_eq!(inventory.tests.len(), 2);
    assert!(inventory
        .tests
        .windows(2)
        .all(|pair| pair[0].declaration_identity <= pair[1].declaration_identity));
    let first = inventory
        .tests
        .iter()
        .find(|test| test.name == "first")
        .expect("first test inventory entry");
    let second = inventory
        .tests
        .iter()
        .find(|test| test.name == "second")
        .expect("second test inventory entry");
    assert_eq!(
        first.declaration_identity,
        test_declaration_id("example.inventory", "first")
    );
    assert!(first.source_span.start < first.source_span.end);
    assert_eq!(first.subject_identity, inventory.subject_identity);
    assert!(inventory
        .tests
        .iter()
        .all(|test| test.qualified_name.starts_with("example.inventory::")));
    assert_eq!(second.name, "second");

    let repeated = front_end(
        "mncs 0.17;\nmodule example.inventory;\n\nfn production() -> (result: i64) { return 7; }\ntest second() -> (result: i64) { return 2; }\ntest first() -> (result: i64) { return 1; }\n",
    )
    .test_inventory
    .expect("repeat inventory");
    assert_eq!(inventory.tests, repeated.tests);
}

#[test]
fn test_case_identity_changes_without_contaminating_production_subject_identity() {
    let before = front_end(
        "mncs 0.17;\nmodule example.identity;\nfn production() -> (result: i64) { return 7; }\ntest evidence() -> (result: i64) { return 1; }\n",
    );
    let after = front_end(
        "mncs 0.17;\nmodule example.identity;\nfn production() -> (result: i64) { return 7; }\ntest evidence() -> (result: i64) { return 2; }\n",
    );
    assert!(before.is_valid(), "{:#?}", before.diagnostics);
    assert!(after.is_valid(), "{:#?}", after.diagnostics);
    let before = before.test_inventory.expect("before inventory");
    let after = after.test_inventory.expect("after inventory");
    assert_eq!(before.subject_identity, after.subject_identity);
    assert_eq!(before.subject_fingerprint, after.subject_fingerprint);
    let before_test = before
        .tests
        .iter()
        .find(|test| test.name == "evidence")
        .expect("before evidence test");
    let after_test = after
        .tests
        .iter()
        .find(|test| test.name == "evidence")
        .expect("after evidence test");
    assert_eq!(
        before_test.declaration_identity,
        after_test.declaration_identity
    );
    assert_ne!(
        before_test.test_case_identity,
        after_test.test_case_identity
    );
    assert_ne!(
        before_test.semantic_fingerprint,
        after_test.semantic_fingerprint
    );
}

#[test]
fn ordinary_declaration_reordering_does_not_change_test_identity() {
    let before = front_end(
        "mncs 0.17;\nmodule example.order;\nfn alpha() -> (result: i64) { return 1; }\ntest evidence() -> (result: i64) { return 2; }\nfn omega() -> (result: i64) { return 3; }\n",
    )
    .test_inventory
    .expect("before inventory");
    let after = front_end(
        "mncs 0.17;\nmodule example.order;\nfn omega() -> (result: i64) { return 3; }\ntest evidence() -> (result: i64) { return 2; }\nfn alpha() -> (result: i64) { return 1; }\n",
    )
    .test_inventory
    .expect("after inventory");
    assert_eq!(before.subject_identity, after.subject_identity);
    assert_eq!(before.subject_fingerprint, after.subject_fingerprint);
    assert_eq!(before.tests, after.tests);
}

#[test]
fn duplicate_first_class_test_names_are_rejected_precisely() {
    let output = front_end(
        "mncs 0.17;\nmodule example.duplicate;\ntest evidence() -> (result: i64) { return 1; }\ntest evidence() -> (result: i64) { return 2; }\n",
    );
    assert!(!output.is_valid());
    assert!(output.test_inventory.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "MNE104"));
}

#[test]
fn invalid_source_has_no_authoritative_inventory() {
    let output = front_end(
        "mncs 0.16;\nmodule example.invalid_inventory;\ntest evidence() -> (result: i64) { return 1; }\n",
    );
    assert!(!output.is_valid());
    assert!(output.test_inventory.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "MNP220"));
}
