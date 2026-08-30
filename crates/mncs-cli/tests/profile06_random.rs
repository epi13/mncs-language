use std::process::Command;

use serde_json::Value;

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn library(name: &str) -> String {
    format!("{}/../../library/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn random_split_is_reproducible_and_backend_agreed() {
    let source = example("source/profile06-random-split.mncs");
    let corpus = example("execution/profile06-random-split-corpus.json");
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
            .expect("run random split experiment");
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
