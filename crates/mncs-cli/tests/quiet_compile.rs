use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/quiet-compile.mncs")
}

fn unresolved_source() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/source/bounded-min.mncs")
}

fn output_dir(label: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("mncs-quiet-compile-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn compile(
    program: &std::path::Path,
    output_dir: &std::path::Path,
    quiet: bool,
    omit_result: bool,
) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mncs"));
    command
        .args([
            "compile",
            program.to_str().expect("source path is UTF-8"),
            "--emit",
            "backend",
            "--target",
            "mncs-research-bytecode",
            "--output-dir",
        ])
        .arg(output_dir);
    if quiet {
        command.arg("--quiet");
    }
    if omit_result {
        command.arg("--omit-result");
    }
    command.output().expect("run compiler")
}

#[test]
fn quiet_artifact_compile_omits_only_requested_duplicate_outputs() {
    let default_dir = output_dir("default");
    let default = compile(&source(), &default_dir, false, false);
    assert!(
        default.status.success(),
        "default compile failed: {}",
        String::from_utf8_lossy(&default.stderr)
    );
    let default_result: serde_json::Value =
        serde_json::from_slice(&default.stdout).expect("default compile keeps JSON output");
    assert_eq!(default_result["status"], "completed");
    assert!(default_dir.join("backend.json").is_file());
    assert!(default_dir.join("result.json").is_file());

    let quiet_dir = output_dir("quiet");
    let quiet = compile(&source(), &quiet_dir, true, true);
    assert!(
        quiet.status.success(),
        "quiet compile failed: {}",
        String::from_utf8_lossy(&quiet.stderr)
    );
    assert!(
        quiet.stdout.is_empty(),
        "completed quiet compile has no JSON stdout"
    );
    assert!(
        quiet.stderr.is_empty(),
        "completed quiet compile has no stderr"
    );
    assert!(quiet_dir.join("backend.json").is_file());
    assert!(!quiet_dir.join("result.json").exists());

    let _ = fs::remove_dir_all(default_dir);
    let _ = fs::remove_dir_all(quiet_dir);
}

#[test]
fn quiet_keeps_unresolved_results_unless_the_result_is_explicitly_omitted() {
    let unresolved = unresolved_source();
    let visible_dir = output_dir("unresolved-visible");
    let visible = compile(&unresolved, &visible_dir, true, false);
    assert!(visible.status.success());
    let result: serde_json::Value =
        serde_json::from_slice(&visible.stdout).expect("unresolved JSON remains visible");
    assert_eq!(result["status"], "completed_with_unresolved_obligations");
    assert!(visible_dir.join("result.json").is_file());

    let omitted_dir = output_dir("unresolved-omitted");
    let omitted = compile(&unresolved, &omitted_dir, true, true);
    assert!(omitted.status.success());
    assert!(omitted.stdout.is_empty());
    assert!(omitted_dir.join("backend.json").is_file());
    assert!(!omitted_dir.join("result.json").exists());

    let _ = fs::remove_dir_all(visible_dir);
    let _ = fs::remove_dir_all(omitted_dir);
}

#[test]
fn quiet_compile_requires_artifact_output_directory() {
    let output = Command::new(env!("CARGO_BIN_EXE_mncs"))
        .args([
            "compile",
            source().to_str().expect("source path is UTF-8"),
            "--quiet",
        ])
        .output()
        .expect("run compiler");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("--quiet requires --output-dir"));
}
