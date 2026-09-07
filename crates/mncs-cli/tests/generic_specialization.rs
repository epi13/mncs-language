use std::process::Command;

use serde_json::Value;

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

/// A generic call nested in a generic iteration body must specialize with
/// an exact call/authority closure (SSA022): the rewritten nested target
/// has to be mirrored into the recorded iteration callees. Regression for
/// the defect found while building `mncs.std.text_scan.v1`.
#[test]
fn nested_generic_call_in_iteration_specializes_on_both_backends() {
    let source = example("source/generic-nested-iteration.mncs");
    let corpus = example("execution/generic-nested-iteration-corpus.json");
    for backend in ["mncs-research-bytecode", "mncs-portable-wasm-mvp"] {
        let output = binary()
            .args(["experiment", "run", &source, "--backend", backend, "--corpus", &corpus])
            .output()
            .expect("run nested-generic experiment");
        assert!(
            output.status.success(),
            "{backend}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
        for case in result["cases"].as_array().unwrap() {
            assert_eq!(case["status"], "returned", "{backend}: {case}");
            assert_eq!(case["expectation_met"], true, "{backend}: {case}");
        }
    }
}
