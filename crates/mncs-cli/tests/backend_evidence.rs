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
        assert_eq!(envelope["exports"], Value::Array(vec!["bounded_min".into()]));
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
