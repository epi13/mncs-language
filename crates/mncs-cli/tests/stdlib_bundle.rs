//! INGEST-P-008: pinned stdlib distribution end to end.
//!
//! `MNCS_STDLIB_BUNDLE` lets a consumer resolve pinned `mncs.std.*`
//! identities with no working-tree stdlib and no vendored copies:
//! bundle-only elaboration and execution, byte-identical collapse with a
//! filesystem tree, fail-closed conflict on divergent content, and
//! refusal of tampered bundles.

use std::fs;
use std::process::Command;

use serde_json::Value;

fn library_dir() -> String {
    format!("{}/../../library", env!("CARGO_MANIFEST_DIR"))
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn workspace(tag: &str) -> std::path::PathBuf {
    let dir =
        std::env::temp_dir().join(format!("mncs-stdlib-bundle-{}-{}", std::process::id(), tag));
    fs::create_dir_all(&dir).expect("workspace directory");
    dir
}

fn consumer_source() -> &'static str {
    "mncs 0.14;\nmodule app.bundleproof;\nuse mncs.std.text_utf8.v1;\nfn check(buf: [byte; 16], n: u64) -> (result: i64) {\n    let view: [byte; up_to 16] = buf[0..n];\n    return validate_generic<16>(view, n);\n}\n"
}

fn source_study(
    source: &std::path::Path,
    library_path: Option<&str>,
    bundle: Option<&str>,
) -> (Option<i32>, Value) {
    let mut command = binary();
    command.args(["source-study", &source.to_string_lossy()]);
    match library_path {
        Some(root) => {
            command.env("MNCS_LIBRARY_PATH", root);
        }
        None => {
            command.env_remove("MNCS_LIBRARY_PATH");
        }
    }
    match bundle {
        Some(path) => {
            command.env("MNCS_STDLIB_BUNDLE", path);
        }
        None => {
            command.env_remove("MNCS_STDLIB_BUNDLE");
        }
    }
    let output = command.output().expect("run source-study");
    let value: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
    (output.status.code(), value)
}

fn diagnostics(value: &Value) -> Vec<Value> {
    value["diagnostics"].as_array().cloned().unwrap_or_default()
}

/// A consumer with no working-tree stdlib and no vendored copies resolves
/// pinned modules (including the transitive `json_cursor` import) from the
/// bundle alone.
#[test]
fn bundle_only_consumer_elaborates_without_filesystem_stdlib() {
    let dir = workspace("only");
    let source_path = dir.join("consumer.mncs");
    fs::write(&source_path, consumer_source()).expect("write consumer");
    let bundle = format!("{}/stdlib-bundle.json", library_dir());
    let (code, study) = source_study(&source_path, None, Some(&bundle));
    assert_eq!(code, Some(0), "study JSON: {study:#}");
    let held = diagnostics(&study);
    let errors: Vec<&Value> = held
        .iter()
        .filter(|diagnostic| diagnostic["severity"] == "error" && diagnostic["code"] != "CMP301")
        .collect();
    assert!(
        errors.is_empty(),
        "bundle-only elaboration has no errors: {errors:?}"
    );
    let declared: Vec<&str> = study["module_resolutions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|resolution| resolution["declared_module"].as_str())
        .collect();
    assert!(
        declared.contains(&"mncs.std.text_utf8.v1"),
        "direct import resolves from the bundle: {declared:?}"
    );
    assert!(
        declared.contains(&"mncs.std.json_cursor.v1"),
        "transitive import resolves from the bundle: {declared:?}"
    );
    fs::remove_dir_all(&dir).ok();
}

/// Byte-identical bundle and filesystem authorities collapse to one: the
/// pin agrees with the tree it was generated from instead of conflicting.
#[test]
fn byte_identical_bundle_and_tree_collapse() {
    let dir = workspace("collapse");
    let source_path = dir.join("consumer.mncs");
    fs::write(&source_path, consumer_source()).expect("write consumer");
    let bundle = format!("{}/stdlib-bundle.json", library_dir());
    let (code, study) = source_study(&source_path, Some(&library_dir()), Some(&bundle));
    assert_eq!(code, Some(0), "study JSON: {study:#}");
    let held = diagnostics(&study);
    let codes: Vec<&str> = held
        .iter()
        .filter_map(|diagnostic| diagnostic["code"].as_str())
        .collect();
    assert!(
        !codes.contains(&"MNE234"),
        "identical authorities must not conflict: {codes:?}"
    );
    fs::remove_dir_all(&dir).ok();
}

/// A divergent pin and tree for one identity fail closed as MNE234 naming
/// both locators — the pin never silently shadows the tree, nor vice
/// versa. The conflict must name the bundle locator so the consumer knows
/// to re-pin.
#[test]
fn divergent_bundle_and_tree_conflict_loudly() {
    let dir = workspace("conflict");
    let tree = dir.join("library");
    copy_dir(std::path::Path::new(&library_dir()), &tree);
    let chunk = tree.join("std/chunk.mncs");
    let text = fs::read_to_string(&chunk).expect("read chunk");
    fs::write(&chunk, text.replacen("find_newline", "find_newlinX", 1)).expect("diverge chunk");
    let edited_bundle = dir.join("edited-bundle.json");
    let generate = binary()
        .args([
            "bundle",
            "generate",
            "--library",
            &tree.to_string_lossy(),
            "--output",
            &edited_bundle.to_string_lossy(),
            "--commit",
            "test-divergent",
        ])
        .output()
        .expect("generate edited bundle");
    assert!(
        generate.status.success(),
        "generate: {}",
        String::from_utf8_lossy(&generate.stderr)
    );
    // The edited tree copy exists only to mint the divergent pin; the
    // consumer resolves against the pristine tree plus the divergent pin.
    let direct_path = dir.join("direct.mncs");
    fs::write(
        &direct_path,
        "mncs 0.13;\nmodule app.direct;\nuse mncs.std.chunk.v1;\nfn probe(view: [byte; up_to 64], n: u64, from: u64) -> (result: i64) {\n    return find_newline(view, n, from);\n}\n",
    )
    .expect("write direct consumer");
    let (_code, direct) = source_study(
        &direct_path,
        Some(&library_dir()),
        Some(&edited_bundle.to_string_lossy()),
    );
    let direct_conflicts: Vec<Value> = diagnostics(&direct)
        .into_iter()
        .filter(|diagnostic| diagnostic["code"] == "MNE234")
        .collect();
    assert_eq!(
        direct_conflicts.len(),
        1,
        "divergent pin and tree must conflict: {:?}",
        diagnostics(&direct)
    );
    let message = direct_conflicts[0]["message"].as_str().unwrap_or("");
    assert!(
        message.contains("stdlib-bundle:"),
        "conflict names the bundle locator: {message}"
    );
    fs::remove_dir_all(&dir).ok();
}

/// Execution through the bundle alone meets the same expectations as the
/// filesystem path: resolution changes provenance, never values.
#[test]
fn bundle_only_execution_meets_filesystem_expectations() {
    for backend in ["mncs-research-bytecode", "mncs-c11"] {
        let output = binary()
            .env_remove("MNCS_LIBRARY_PATH")
            .env(
                "MNCS_STDLIB_BUNDLE",
                format!("{}/stdlib-bundle.json", library_dir()),
            )
            .args([
                "experiment",
                "run",
                &format!("{}/std/text_utf8.mncs", library_dir()),
                "--backend",
                backend,
                "--corpus",
                &example("execution/text-utf8-corpus.json"),
            ])
            .output()
            .expect("run bundle experiment");
        assert!(
            output.status.success(),
            "{backend}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
        let cases = result["cases"].as_array().expect("cases");
        assert_eq!(cases.len(), 17, "{backend}: case count");
        for case in cases {
            let id = case["case_id"].as_str().unwrap_or("?");
            assert_eq!(case["status"], "returned", "{backend} {id}: {case:#}");
            assert_eq!(case["expectation_met"], true, "{backend} {id}: {case:#}");
        }
    }
}

/// A tampered bundle file refuses verification with a nonzero exit.
#[test]
fn tampered_bundle_fails_verification() {
    let dir = workspace("tamper");
    let pristine =
        fs::read_to_string(format!("{}/stdlib-bundle.json", library_dir())).expect("read pin");
    let mut document: Value = serde_json::from_str(&pristine).expect("pin parses");
    let modules = document["modules"].as_array_mut().expect("modules");
    let entry = modules
        .iter_mut()
        .find(|module| module["name"] == "mncs.std.chunk.v1")
        .expect("chunk entry");
    let text = entry["text"].as_str().expect("text").to_owned();
    entry["text"] = Value::String(text.replacen("find_newline", "find_newlinX", 1));
    let tampered = dir.join("tampered.json");
    fs::write(
        &tampered,
        serde_json::to_string(&document).expect("reserialize"),
    )
    .expect("write tampered");
    let output = binary()
        .args(["bundle", "verify", &tampered.to_string_lossy()])
        .output()
        .expect("verify tampered");
    assert!(
        !output.status.success(),
        "tampered bundle must fail verification"
    );
    let ok = binary()
        .args([
            "bundle",
            "verify",
            &format!("{}/stdlib-bundle.json", library_dir()),
        ])
        .output()
        .expect("verify pristine");
    assert!(
        ok.status.success(),
        "pristine bundle verifies: {}",
        String::from_utf8_lossy(&ok.stderr)
    );
    fs::remove_dir_all(&dir).ok();
}

fn copy_dir(source: &std::path::Path, target: &std::path::Path) {
    fs::create_dir_all(target).expect("create target");
    let mut entries: Vec<_> = fs::read_dir(source)
        .expect("read source")
        .collect::<Result<Vec<_>, _>>()
        .expect("list source");
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        let destination = target.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &destination);
        } else {
            fs::copy(&path, &destination).expect("copy file");
        }
    }
}
