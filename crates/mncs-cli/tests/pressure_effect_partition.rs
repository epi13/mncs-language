//! P1-B02 partial realization: per-entrypoint admission on compiled
//! backends (acceptance 13, and the declaration half of 12).
//!
//! A module that hosts effects no longer poisons its pure neighbors: each
//! function lowers independently, callers of refused functions are refused
//! transitively (so no admitted function references a missing entry), the
//! artifact realizes the admitted subset (`exports`) and records every
//! refusal in `unsupported`, and drivers gate entry lookup on `exports`
//! so refused entrypoints fail closed as Unsupported instead of
//! mislinking. A module with nothing realizable still refuses
//! whole-program exactly as before.
//!
//! The corpus module mixes both halves: `frame_add` / `pure_caller` are
//! pure (admitted everywhere); `append_word` needs `host_write`
//! (bytecode with `--grant-write` realizes it; compiled backends refuse
//! the entrypoint); `append_and_add` calls `append_word`, so its own pure
//! body is still refused transitively through its closure.

use std::process::Command;

use serde_json::Value;

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn library(name: &str) -> String {
    format!("{}/../../library/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn run_experiment(
    source: &str,
    backend: &str,
    corpus: &str,
    grant: Option<(&std::path::Path, &str)>,
) -> Value {
    let mut command = binary();
    command
        .args([
            "experiment",
            "run",
            source,
            "--backend",
            backend,
            "--corpus",
            corpus,
        ])
        .env("MNCS_LIBRARY_PATH", library(""));
    if let Some((path, capability)) = grant {
        command.args([
            "--grant-write",
            &format!("{capability}={}", path.to_string_lossy()),
        ]);
    }
    let output = command.output().expect("run experiment");
    serde_json::from_slice(&output.stdout).expect("experiment JSON")
}

fn workspace(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "mncs-effect-partition-{}-{}-{:?}",
        tag,
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("create workspace");
    let ledger = dir.join("ledger.bin");
    (dir, ledger)
}

const COMPILED_BACKENDS: [&str; 4] = [
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

/// On every compiled backend the pure entrypoints return with their
/// expectations met while the effectful entrypoints (direct and
/// transitive) fail closed as Unsupported with the admission reason —
/// never a trap, a value, or a mislink.
#[test]
fn pure_entrypoints_survive_effect_neighbors_on_compiled_backends() {
    let source = example("source/pressure-effect-partition.mncs");
    let corpus = example("execution/pressure-effect-partition-corpus.json");
    for backend in COMPILED_BACKENDS {
        let (_dir, ledger) = workspace(backend);
        let result = run_experiment(&source, backend, &corpus, Some((&ledger, "ledger_writer")));
        let cases = result["cases"]
            .as_array()
            .unwrap_or_else(|| panic!("{backend}: missing cases; {result:#}"));
        assert_eq!(cases.len(), 4, "{backend}: case count");
        for case in cases {
            let id = case["case_id"].as_str().unwrap_or("?");
            if id.starts_with("pure-") {
                assert_eq!(case["status"], "returned", "{backend} {id}: {case:#}");
                assert_eq!(
                    case["expectation_met"], true,
                    "{backend} {id}: pure value mismatch; {case:#}"
                );
            } else {
                assert_eq!(
                    case["status"], "unsupported",
                    "{backend} {id}: effectful entry must fail closed; {case:#}"
                );
                let reason = case["failure_reason"].as_str().unwrap_or("");
                assert!(
                    reason.contains("not realized by this artifact"),
                    "{backend} {id}: refusal must cite admission; {case:#}"
                );
            }
        }
        // Refused entrypoints never touch the destination: the grant is
        // presented but no compiled effect realizes.
        assert!(
            !ledger.exists(),
            "{backend}: refused entrypoints must not create the destination"
        );
    }
}

/// The research backend with a grant realizes the whole module: pure and
/// effectful entrypoints all return, and both appends land byte-exact.
#[test]
fn granted_bytecode_realizes_the_whole_module() {
    let source = example("source/pressure-effect-partition.mncs");
    let corpus = example("execution/pressure-effect-partition-corpus.json");
    let (_dir, ledger) = workspace("bytecode");
    let result = run_experiment(
        &source,
        "mncs-research-bytecode",
        &corpus,
        Some((&ledger, "ledger_writer")),
    );
    for case in result["cases"].as_array().unwrap() {
        let id = case["case_id"].as_str().unwrap_or("?");
        assert_eq!(case["status"], "returned", "bytecode {id}: {case:#}");
        assert_eq!(case["expectation_met"], true, "bytecode {id}: {case:#}");
    }
    assert_eq!(
        std::fs::read(&ledger).expect("read ledger"),
        b"abcdabcd",
        "both granted appends land byte-exact"
    );
}

/// The artifact itself is the machine-readable admission report: admitted
/// entrypoints in `exports`, every refusal (direct host-call and
/// transitive closure) in `unsupported` with the requiring function named.
#[test]
fn artifact_carries_the_admission_report() {
    let source = example("source/pressure-effect-partition.mncs");
    let corpus = example("execution/pressure-effect-partition-corpus.json");
    let (_dir, ledger) = workspace("report");
    let result = run_experiment(
        &source,
        "mncs-c11",
        &corpus,
        Some((&ledger, "ledger_writer")),
    );
    let artifact = &result["artifact"];
    let exports: Vec<String> = artifact["exports"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect();
    assert_eq!(exports.len(), 2, "admitted subset: {exports:?}");
    assert!(
        exports.iter().any(|export| export.contains("frame__add")),
        "pure frame_add is exported: {exports:?}"
    );
    assert!(
        exports.iter().any(|export| export.contains("pure__caller")),
        "pure caller is exported: {exports:?}"
    );
    assert!(
        !exports.iter().any(|export| export.contains("append")),
        "effectful entries are never exported: {exports:?}"
    );
    let unsupported: Vec<String> = artifact["unsupported"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect();
    assert_eq!(unsupported.len(), 2, "refusal report: {unsupported:?}");
    assert!(
        unsupported
            .iter()
            .any(|reason| reason.contains("append_word") && reason.contains("host call")),
        "direct host-call refusal names append_word: {unsupported:?}"
    );
    assert!(
        unsupported
            .iter()
            .any(|reason| reason.contains("append_and_add")
                && reason.contains("transitively requires refused")
                && reason.contains("append_word")),
        "transitive refusal names caller and refused callee: {unsupported:?}"
    );
}

/// An all-effectful module still refuses whole-program exactly as before:
/// no artifact, explicit CGN/CGC refusal diagnostics, destination
/// untouched.
#[test]
fn all_effectful_module_still_refuses_whole_program() {
    let source = example("source/host-write-append.mncs");
    let corpus = example("execution/host-write-append-corpus.json");
    let (_dir, ledger) = workspace("allrefused");
    let output = binary()
        .args([
            "experiment",
            "run",
            &source,
            "--backend",
            "mncs-c11",
            "--corpus",
            &corpus,
            "--grant-write",
            &format!("ledger_writer={}", ledger.to_string_lossy()),
        ])
        .env("MNCS_LIBRARY_PATH", library(""))
        .output()
        .expect("run experiment");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        stdout.contains("outside the C11 scalar envelope")
            || stdout.contains("host calls are unsupported"),
        "whole-program refusal stays explicit: {stdout:.600}"
    );
    assert!(
        !ledger.exists(),
        "refused backends must not touch the destination"
    );
}
