//! LLVM branch-condition hygiene: one source binding may guard several
//! branches in a single body, and each emitted branch test needs its own
//! LLVM temporary.
//!
//! The backend once named the condition temporary after the source value
//! (`%c_<name>`), so a second `if` on the same binding redefined one LLVM
//! local and clang rejected the module (`multiple definition of local
//! value`). The temporary is now keyed by the emission counter. The four
//! cases pin hand-computed values on every executable backend; the
//! `mncs-llvm-ir` leg is the regression (it refused as `unsupported`
//! before the fix).

use std::process::Command;

use serde_json::Value;

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

#[test]
fn repeated_branch_binding_agrees_on_every_executable_backend() {
    let source = example("source/pressure-branch-hygiene.mncs");
    let corpus = example("execution/pressure-branch-hygiene-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let output = binary()
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
            .expect("run branch-hygiene experiment");
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{backend}: unexpected exit; {stderr}"
        );
        let result: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("{backend}: experiment JSON ({stderr}): {error}"));
        let cases = result["cases"]
            .as_array()
            .unwrap_or_else(|| panic!("{backend}: missing cases; {result:#}"));
        assert_eq!(cases.len(), 4, "{backend}: case count");
        for case in cases {
            let id = case["case_id"].as_str().unwrap_or("?");
            assert_eq!(
                case["status"], "returned",
                "{backend} {id}: status {case:#}"
            );
            assert_eq!(
                case["expectation_met"], true,
                "{backend} {id}: logical value mismatch; returned={:#}",
                case["returned"]
            );
        }
    }
}
