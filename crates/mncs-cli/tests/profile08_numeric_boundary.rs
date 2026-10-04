use std::process::Command;

use serde_json::Value;

/// Stage F: the `mncs-stdlib` checkout backing these tests: explicit
/// `MNCS_STDLIB_ROOT` wins, else the `mncs-stdlib` sibling checkout.
/// Fails closed with a clear message when absent.
fn stdlib_checkout_dir() -> String {
    // Test inputs need a real checkout: an explicitly empty variable
    // (the CLI's hermetic spelling) falls through to the sibling here.
    let explicit = std::env::var("MNCS_STDLIB_ROOT")
        .ok()
        .filter(|root| !root.trim().is_empty());
    let checkout =
        explicit.unwrap_or_else(|| format!("{}/../../../mncs-stdlib", env!("CARGO_MANIFEST_DIR")));
    assert!(
        std::path::Path::new(&checkout).is_dir(),
        "mncs-stdlib checkout missing at {checkout}; set MNCS_STDLIB_ROOT"
    );
    checkout
}

/// Stage F: standard-library sources now live in `mncs-stdlib/library/`.
fn stdlib_library_dir() -> String {
    let checkout = stdlib_checkout_dir();
    let dir = format!("{checkout}/library");
    assert!(
        std::path::Path::new(&dir).is_dir(),
        "mncs-stdlib checkout missing at {checkout}; set MNCS_STDLIB_ROOT"
    );
    dir
}

/// Stage F: stdlib-owned examples (fixtures/corpora) read across repos.
fn stdlib_example(name: &str) -> String {
    format!("{}/examples/{name}", stdlib_checkout_dir())
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn library(name: &str) -> String {
    format!("{}/{name}", stdlib_library_dir())
}

#[test]
fn eight_scalar_numeric_kernel_crosses_every_executable_backend() {
    let source = stdlib_example("source/profile08-numeric-boundary.mncs");
    let corpus = stdlib_example("execution/profile08-numeric-boundary-corpus.json");
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_mncs"))
            .env("MNCS_LIBRARY_PATH", library(""))
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
            .expect("run numeric boundary experiment");
        let result: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "{backend}: {error}; {}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
        assert_eq!(output.status.code(), Some(0), "{backend}: {result:#}");
        assert!(
            result["cases"]
                .as_array()
                .unwrap()
                .iter()
                .all(|case_| { case_["status"] == "returned" && case_["expectation_met"] == true }),
            "{backend}: {result:#}"
        );
    }
}
