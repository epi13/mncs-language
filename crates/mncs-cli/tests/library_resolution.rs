use std::fs;
use std::process::Command;

use serde_json::Value;

fn library_dir() -> String {
    format!("{}/../../library", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// A consumer outside the library tree must resolve `mncs.core.status.v1`
/// when the library root is exported through `MNCS_LIBRARY_PATH`, and must
/// fail closed with a resolution diagnostic when it is not.
#[test]
fn external_consumer_resolves_core_through_library_path() {
    let workspace = std::env::temp_dir().join(format!("mncs-library-path-{}", std::process::id()));
    fs::create_dir_all(&workspace).expect("workspace directory");
    let source_path = workspace.join("consumer.mncs");
    fs::write(
        &source_path,
        r#"mncs 0.6;

module tmp.consumer;

use mncs.core.status.v1;

fn soften(left: Status, right: Status) -> (result: Status) {
    return dominate(left, right);
}

"#,
    )
    .expect("write consumer");

    // Without a library root the import cannot be satisfied: elaboration
    // fails closed with the unresolvable-import diagnostic.
    let bare = binary()
        .args(["validate"])
        .arg(&source_path)
        .output()
        .expect("validate without library path");
    let bare_text = String::from_utf8_lossy(&bare.stdout);
    assert!(
        !bare.status.success() && bare_text.contains("MNE173"),
        "expected MNE173 without MNCS_LIBRARY_PATH: {bare_text}"
    );

    // With the root exported, the same source elaborates and validates.
    let resolved = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(["validate"])
        .arg(&source_path)
        .output()
        .expect("validate with library path");
    assert!(
        resolved.status.success(),
        "expected success with MNCS_LIBRARY_PATH: {}",
        String::from_utf8_lossy(&resolved.stdout)
    );

    fs::remove_dir_all(&workspace).ok();
}

#[test]
fn external_consumer_executes_linked_stdlib_across_reference_backends() {
    let source = example("source/library-consumer.mncs");
    let corpus = example("execution/library-consumer-corpus.json");
    let library_root = library_dir();

    let validation = binary()
        .env("MNCS_LIBRARY_PATH", &library_root)
        .args(["source-study", &source])
        .output()
        .expect("validate external consumer");
    assert!(validation.status.success());
    let validation_json: Value =
        serde_json::from_slice(&validation.stdout).expect("validation JSON");
    assert_eq!(
        validation_json["module_resolutions"][0]["requested_module"],
        "mncs.core.ordering.v1"
    );
    assert_eq!(
        validation_json["module_resolutions"][0]["declared_module"],
        "mncs.core.ordering.v1"
    );
    assert!(validation_json["module_resolutions"][0]["semantic_fingerprint"].is_string());

    for backend in ["mncs-research-bytecode", "mncs-portable-wasm-mvp"] {
        let output = binary()
            .env("MNCS_LIBRARY_PATH", &library_root)
            .args([
                "experiment",
                "run",
                &source,
                "--backend",
                backend,
                "--corpus",
                &corpus,
            ])
            .output()
            .expect("run linked consumer experiment");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let result: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("experiment JSON ({stderr}): {error}"));
        assert!(output.status.success(), "{backend}: {result:#?}");
        assert_eq!(result["status"], "PASS", "{backend}: {result:#?}");
        assert!(result["cases"]
            .as_array()
            .expect("case observations")
            .iter()
            .all(|case_| case_["expectation_met"] == true));
    }
}

#[test]
fn profile09_consumer_disambiguates_duplicate_core_exports() {
    let source = example("source/profile09-stdlib-namespace-consumer.mncs");
    let corpus = example("execution/profile09-stdlib-namespace-consumer-corpus.json");
    let library_root = library_dir();
    let study = binary()
        .env("MNCS_LIBRARY_PATH", &library_root)
        .args(["source-study", &source])
        .output()
        .expect("run profile 0.9 source study");
    assert!(
        study.status.success(),
        "{}",
        String::from_utf8_lossy(&study.stdout)
    );
    let study_json: Value = serde_json::from_slice(&study.stdout).expect("source-study JSON");
    assert_eq!(study_json["binding_table"]["schema_version"], "0.1");
    assert!(study_json["binding_table"]["references"]
        .as_array()
        .expect("binding references")
        .iter()
        .any(|reference| reference["provenance"] == "aliased"
            && reference["path"]
                .as_array()
                .is_some_and(|path| path.iter().any(|part| part == "mncs.core.ordering.v1"))));

    for backend in ["mncs-research-bytecode", "mncs-portable-wasm-mvp"] {
        let output = binary()
            .env("MNCS_LIBRARY_PATH", &library_root)
            .args([
                "experiment",
                "run",
                &source,
                "--backend",
                backend,
                "--corpus",
                &corpus,
            ])
            .output()
            .expect("execute profile 0.9 stdlib consumer");
        let result: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "{backend}: {error} ({})",
                String::from_utf8_lossy(&output.stderr)
            )
        });
        assert!(output.status.success(), "{backend}: {result:#?}");
        assert_eq!(result["status"], "PASS", "{backend}: {result:#?}");
        assert!(result["cases"]
            .as_array()
            .expect("profile 0.9 case observations")
            .iter()
            .all(|case_| case_["expectation_met"] == true));
    }
}

/// Duplicate module identities fail closed (HARNESS-PRESSURE-008): two
/// library roots offering byte-distinct sources for one module name are
/// rejected with `MNE234`, while byte-identical copies remain one
/// authority and resolve normally.
#[test]
fn conflicting_module_identities_are_rejected() {
    let workspace = std::env::temp_dir().join(format!("mncs-resolution-conflict-{}", std::process::id()));
    let root_a = workspace.join("root-a");
    let root_b = workspace.join("root-b");
    let work = workspace.join("work");
    for dir in [&root_a, &root_b, &work] {
        fs::create_dir_all(dir.join("mncs").join("core")).or_else(|_| fs::create_dir_all(dir)).expect("dir");
    }
    // Root A carries the canonical module; root B carries a divergent copy
    // under the same declared identity.
    let canonical = fs::read_to_string(format!("{}/core/status.mncs", library_dir()))
        .expect("canonical status module");
    fs::write(root_a.join("mncs").join("core").join("status.mncs"), &canonical)
        .expect("write root A");
    fs::write(
        root_b.join("mncs").join("core").join("status.mncs"),
        format!("{canonical}\n// divergent downstream copy\n"),
    )
    .expect("write root B");
    let consumer = work.join("consumer.mncs");
    fs::write(
        &consumer,
        "mncs 0.6;\n\nmodule tmp.consumer;\n\nuse mncs.core.status.v1;\n\nfn check(left: Status, right: Status) -> (result: Status) {\n    return dominate(left, right);\n}\n",
    )
    .expect("write consumer");

    let library_path = format!(
        "{}:{}",
        root_a.display(),
        root_b.display()
    );
    let conflicted = binary()
        .env("MNCS_LIBRARY_PATH", &library_path)
        .args(["source-study"])
        .arg(&consumer)
        .output()
        .expect("source-study with conflicting roots");
    let study: Value =
        serde_json::from_slice(&conflicted.stdout).expect("source-study JSON");
    let codes: Vec<&str> = study["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|diag| diag["code"].as_str())
        .collect();
    assert!(
        codes.contains(&"MNE234"),
        "expected MNE234 for conflicting identities in {codes:?}"
    );

    // Byte-identical copies are one authority: same content in both roots
    // resolves and validates.
    fs::write(root_b.join("mncs").join("core").join("status.mncs"), &canonical)
        .expect("align root B");
    let aligned = binary()
        .env("MNCS_LIBRARY_PATH", &library_path)
        .args(["validate"])
        .arg(&consumer)
        .output()
        .expect("validate with identical copies");
    assert!(
        aligned.status.success(),
        "expected success for identical copies: {}",
        String::from_utf8_lossy(&aligned.stdout)
    );

    fs::remove_dir_all(&workspace).ok();
}
