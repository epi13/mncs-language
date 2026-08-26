use std::process::Command;

use serde_json::Value;

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn run(source: &str, backend: &str, corpus: &str) -> (Option<i32>, Value) {
    let output = binary()
        .args([
            "experiment",
            "run",
            source,
            "--backend",
            backend,
            "--corpus",
            corpus,
            "--output-dir",
            "/tmp",
        ])
        .output()
        .expect("run experiment");
    let value: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{backend}: non-JSON result ({error})"));
    (output.status.code(), value)
}

/// Explicit arithmetic intents (`+% -% *%` wrapping, `+| -| *|` saturating)
/// must agree with the reference executors on every realizing backend over
/// the boundary corpus, including i64 saturation edges and narrow wrapping
/// wraparound.
#[test]
fn explicit_intents_agree_across_executable_backends() {
    let source = example("source/profile06-explicit-intents.mncs");
    let corpus = example("execution/profile06-explicit-intents-corpus.json");
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "c11",
        "llvm",
        "cranelift",
    ] {
        let (_, result) = run(&source, backend, &corpus);
        assert_eq!(
            result["status"], "PASS",
            "{backend}: {:#?}",
            result["cases"]
        );
        assert!(result["cases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|case_| case_["expectation_met"] == true));
    }
}

/// In profiles below 0.6 the intent operators fail closed with a precise
/// diagnostic instead of silently binding to default checked semantics.
#[test]
fn explicit_intents_require_profile_0_6() {
    let workspace = std::env::temp_dir().join(format!("mncs-intents-gate-{}", std::process::id()));
    std::fs::create_dir_all(&workspace).expect("workspace directory");
    let source_path = workspace.join("legacy.mncs");
    std::fs::write(
        &source_path,
        r#"mncs 0.5;

module legacy.counters;

fn bump(count: i64) -> (result: i64) {
    return count +| 1;
}
"#,
    )
    .expect("write legacy source");
    let output = binary()
        .args(["validate"])
        .arg(&source_path)
        .output()
        .expect("validate legacy source");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success() && text.contains("MNP143"),
        "expected MNP143 below 0.6: {text}"
    );
    std::fs::remove_dir_all(&workspace).ok();
}
