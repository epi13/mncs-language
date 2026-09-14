use std::process::Command;

use serde_json::Value;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn unique_root() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "mncs-test-artifact-policy-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos()
    ))
}

fn exports(source: &std::path::Path, output: &std::path::Path, include_tests: bool) -> Vec<String> {
    let mut command = binary();
    command.args([
        "compile",
        source.to_str().expect("source path"),
        "--emit",
        "backend",
        "--target",
        "research-bytecode",
        "--output-dir",
        output.to_str().expect("output path"),
    ]);
    if include_tests {
        command.arg("--include-tests");
    } else {
        command.arg("--exclude-tests");
    }
    let result = command.output().expect("run compiler");
    assert!(
        result.status.success(),
        "compiler failed: {}",
        String::from_utf8_lossy(&result.stdout)
    );
    let backend: Value = serde_json::from_slice(
        &std::fs::read(output.join("backend.json")).expect("backend artifact"),
    )
    .expect("backend JSON");
    backend["exports"]
        .as_array()
        .expect("backend exports")
        .iter()
        .map(|value| value.as_str().expect("export name").to_owned())
        .collect()
}

#[test]
fn production_and_test_artifact_policies_are_explicit() {
    let root = unique_root();
    std::fs::create_dir_all(&root).expect("create artifact policy fixture");
    let source = root.join("policy.mncs");
    std::fs::write(
        &source,
        "mncs 0.17;\nmodule policy;\nfn production() -> (result: i64) { return 7; }\ntest evidence() -> (result: i64) { return 1; }\n",
    )
    .expect("write policy source");

    let production = exports(&source, &root.join("production"), false);
    let test_artifact = exports(&source, &root.join("tests"), true);
    assert_eq!(production, vec!["production"]);
    assert_eq!(test_artifact, vec!["evidence", "production"]);

    std::fs::remove_dir_all(&root).expect("remove exact artifact policy fixture");
}
