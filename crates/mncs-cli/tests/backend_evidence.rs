//! Backend evidence honesty and cross-backend agreement for external targets.
//!
//! The `mncs-riscv32`, `mncs-ebpf`, and `mncs-ptx64` backends lower through
//! the system LLVM toolchain. This suite pins three properties that must
//! hold wherever `llc` exists, and asserts structured toolchain errors
//! where it does not:
//!
//! 1. Every external artifact envelope states its execution boundary in
//!    `assumptions` (compile-only stays compile-only).
//! 2. Artifact bytes are deterministic across repeated compilation.
//! 3. All three backends agree on the function value contract for the same
//!    MNCS source (differential pressure at the contract level).

use std::process::Command;

use serde_json::Value;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn program() -> String {
    format!(
        "{}/../../examples/source/bounded-min.mncs",
        env!("CARGO_MANIFEST_DIR")
    )
}

fn out_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mncs-backend-evidence-{}-{}",
        tag,
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create output dir");
    dir
}

fn compile_backend(target: &str, dir: &std::path::Path) -> Option<Value> {
    let output = binary()
        .args([
            "compile",
            &program(),
            "--emit",
            "backend",
            "--target",
            target,
            "--output-dir",
        ])
        .arg(dir)
        .output()
        .expect("run compile");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stderr.contains("CGX401") || stdout.contains("CGX401"),
            "backend failure without llc must carry CGX401, got: {stderr} {stdout}"
        );
        return None;
    }
    let envelope: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("backend.json")).expect("backend.json"),
    )
    .expect("backend envelope JSON");
    Some(envelope)
}

fn boundary_terms(target: &str) -> &'static str {
    match target {
        "mncs-riscv32" => "emulator",
        "mncs-ebpf" => "verifier",
        "mncs-ptx64" => "GPU",
        _ => unreachable!(),
    }
}

#[test]
fn external_backends_state_their_execution_boundary() {
    let mut proven = 0;
    for target in ["mncs-riscv32", "mncs-ebpf", "mncs-ptx64"] {
        let dir = out_dir(&format!("boundary-{target}"));
        let Some(envelope) = compile_backend(target, &dir) else {
            continue;
        };
        proven += 1;
        assert_eq!(envelope["status"], "PASS", "{target} envelope status");
        assert_eq!(
            envelope["exports"],
            Value::Array(vec!["bounded_min".into()])
        );
        let sha = envelope["bytes_sha256"].as_str().expect("bytes_sha256");
        assert_eq!(sha.len(), 64, "{target} sha256 length");
        let assumptions = envelope["assumptions"]
            .as_array()
            .expect("assumptions array");
        assert!(
            assumptions.iter().any(|a| a
                .as_str()
                .unwrap_or_default()
                .contains(boundary_terms(target))),
            "{target} assumptions must name its missing execution step"
        );
    }
    if proven == 0 {
        eprintln!("SKIP-BUT-STRUCTURED: no llc on PATH; CGX401 asserted instead");
    }
}

#[test]
fn external_artifacts_are_deterministic() {
    let first = out_dir("determinism-a");
    let second = out_dir("determinism-b");
    let (Some(a), Some(b)) = (
        compile_backend("mncs-riscv32", &first),
        compile_backend("mncs-riscv32", &second),
    ) else {
        return;
    };
    assert_eq!(
        a["bytes_sha256"], b["bytes_sha256"],
        "repeated compilation must produce identical artifact bytes"
    );
    assert_eq!(
        a["identity"], b["identity"],
        "repeated compilation must produce identical artifact identity"
    );
}

fn compile_backend_with_entries(
    target: &str,
    entries: &[&str],
    dir: &std::path::Path,
) -> Option<Value> {
    let mut command = binary();
    command.args([
        "compile",
        &program(),
        "--emit",
        "backend",
        "--target",
        target,
    ]);
    for entry in entries {
        command.arg("--entry").arg(entry);
    }
    let output = command
        .arg("--output-dir")
        .arg(dir)
        .output()
        .expect("run compile");
    if !output.status.success() {
        return None;
    }
    let envelope: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("backend.json")).expect("backend.json"),
    )
    .expect("backend envelope JSON");
    Some(envelope)
}

fn envelope_text(envelope: &Value, dir: &std::path::Path) -> String {
    let hex = envelope["bytes_hex"].as_str().expect("bytes_hex");
    assert_eq!(hex.len() % 2, 0, "bytes_hex must decode");
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex digit"))
        .collect();
    let path = dir.join("artifact.bin");
    std::fs::write(&path, &bytes).expect("write artifact bytes");
    String::from_utf8_lossy(&bytes).into_owned()
}

#[test]
fn ptx_entry_selection_emits_launchable_kernel() {
    let dir = out_dir("ptx-entry");
    let Some(envelope) = compile_backend_with_entries("mncs-ptx64", &["bounded_min"], &dir) else {
        return;
    };
    assert_eq!(envelope["status"], "PASS", "entry compile envelope status");
    let text = envelope_text(&envelope, &dir);
    assert!(
        text.contains(".visible .entry bounded_min("),
        "selected entry must lower to a launchable .entry"
    );
    assert!(
        !text.contains(".visible .func bounded_min("),
        "selected entry must not remain a plain .func"
    );
    let applicability = envelope["execution_applicability"]
        .as_array()
        .expect("execution_applicability array");
    assert!(
        applicability.iter().any(|entry| entry
            .as_str()
            .unwrap_or_default()
            .contains("launchable ptx kernel entries: bounded_min")),
        "envelope must record the kernel selection machine-readably"
    );
}

#[test]
fn ptx_without_entry_stays_callable_device_function() {
    let dir = out_dir("ptx-plain");
    let Some(envelope) = compile_backend("mncs-ptx64", &dir) else {
        return;
    };
    let text = envelope_text(&envelope, &dir);
    assert!(
        text.contains(".visible .func bounded_min("),
        "no entry selected: export stays a callable .func"
    );
    assert!(
        !text.contains(".visible .entry bounded_min("),
        "no entry selected: no launchable .entry may appear"
    );
}

#[test]
fn ptx_unknown_entry_is_a_structured_error() {
    let dir = out_dir("ptx-bad-entry");
    let output = binary()
        .args([
            "compile",
            &program(),
            "--emit",
            "backend",
            "--target",
            "mncs-ptx64",
            "--entry",
            "no_such_export",
            "--output-dir",
        ])
        .arg(&dir)
        .output()
        .expect("run compile");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("CGX303"),
        "unknown entry must fail with CGX303, got: {}",
        &stdout[..stdout.len().min(600)]
    );
    assert!(
        !dir.join("backend.json").exists(),
        "unknown entry must not emit an artifact"
    );
}

#[test]
fn entry_on_non_ptx_target_is_rejected() {
    let output = binary()
        .args([
            "compile",
            &program(),
            "--emit",
            "backend",
            "--target",
            "mncs-riscv32",
            "--entry",
            "bounded_min",
            "--output-dir",
        ])
        .arg(out_dir("entry-riscv-reject"))
        .output()
        .expect("run compile");
    assert!(
        !output.status.success(),
        "--entry on a non-PTX target must be rejected, not ignored"
    );
}

#[test]
fn external_backends_agree_on_the_value_contract() {
    let mut contracts = Vec::new();
    for target in ["mncs-riscv32", "mncs-ebpf", "mncs-ptx64"] {
        let dir = out_dir(&format!("agree-{target}"));
        if let Some(envelope) = compile_backend(target, &dir) {
            contracts.push(envelope["function_value_contracts"].clone());
        }
    }
    if contracts.len() < 2 {
        return;
    }
    for other in &contracts[1..] {
        assert_eq!(
            &contracts[0], other,
            "all backends must agree on the bounded_min value contract"
        );
    }
}
