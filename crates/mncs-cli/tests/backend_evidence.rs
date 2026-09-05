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
    if !dir.join("backend.json").exists() {
        assert_structured_refusal(dir, target);
        return None;
    }
    let envelope: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("backend.json")).expect("backend.json"),
    )
    .expect("backend envelope JSON");
    Some(envelope)
}

/// Assert the honest structured refusal when no backend artifact was
/// produced (issue #110). Toolchain absence is Unknown, not failure: the
/// compiler exits 0 with unresolved obligations and no `backend.json`,
/// and the refusal lives machine-readably in `result.json` diagnostics
/// (`CGX401` / `external_tool_failure`). This path must never panic:
/// absence of `llc` is a finding, not a harness crash.
fn assert_structured_refusal(dir: &std::path::Path, target: &str) {
    let result: Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("result.json")).expect("result.json carries the refusal"),
    )
    .expect("result JSON");
    assert_ne!(
        result["status"], "completed",
        "{target}: a missing artifact with a Pass status would be a lie"
    );
    let diagnostics = result["diagnostics"].as_array().expect("diagnostics array");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["code"] == "CGX401"
                && diagnostic["kind"] == "external_tool_failure"),
        "{target}: refusal must carry structured CGX401/external_tool_failure, got: {diagnostics:?}"
    );
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
    if !dir.join("backend.json").exists() {
        assert_structured_refusal(dir, target);
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

fn have_tool(name: &str) -> bool {
    std::process::Command::new(name)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn reference_min(dir: &std::path::Path, a: i64, b: i64) -> i64 {
    let request = serde_json::json!({
        "schema_version": "0.1",
        "target": {"module": "examples.concepts", "function": "bounded_min"},
        "arguments": [
            {"integer": {"value": a, "type": {"bits": 32, "signed": true}}},
            {"integer": {"value": b, "type": {"bits": 32, "signed": true}}},
        ],
        "step_budget": 100000,
    });
    let request_path = dir.join(format!("request-{a}-{b}.json"));
    std::fs::write(&request_path, serde_json::to_string(&request).expect("request JSON"))
        .expect("write request");
    let output = binary()
        .args(["execute", &program()])
        .arg(&request_path)
        .output()
        .expect("run reference execute");
    assert!(
        output.status.success(),
        "reference execution must succeed for ({a}, {b})"
    );
    let result: Value =
        serde_json::from_slice(&output.stdout).expect("reference result JSON");
    assert_eq!(result["status"], "returned", "reference must return");
    result["returned"][0]["integer"]["value"]
        .as_i64()
        .expect("reference integer result")
}

/// Real RV32 execution: the MNCS riscv32 backend object, freestanding-linked
/// with a minimal `_start` (no libc), run under qemu-riscv32, compared case
/// by case against the MNCS reference executor. Toolchain absence is
/// Unknown, never failure: each missing piece skips honestly.
#[test]
fn riscv32_qemu_executes_bounded_min() {
    for tool in ["clang", "qemu-riscv32"] {
        if !have_tool(tool) {
            eprintln!("SKIP-BUT-STRUCTURED: {tool} is not installed; no RISC-V emulator run");
            return;
        }
    }
    let dir = out_dir("riscv32-qemu");
    let Some(envelope) = compile_backend("mncs-riscv32", &dir) else {
        return;
    };
    assert_eq!(envelope["status"], "PASS", "riscv32 envelope status");
    let hex = envelope["bytes_hex"].as_str().expect("bytes_hex");
    assert_eq!(hex.len() % 2, 0, "bytes_hex must decode");
    let object: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("hex digit"))
        .collect();
    std::fs::write(dir.join("bounded_min.o"), &object).expect("write object");

    for (a, b) in [(200i64, 77i64), (17, 42), (5, 5)] {
        let start = format!(
            "    .globl _start\n    .text\n_start:\n    addi sp, sp, -32\n    mv s0, sp\n    li a0, {a}\n    li a1, {b}\n    mv a2, s0\n    mv a3, s0\n    call bounded_min\n    mv t0, a0\n    li a7, 93\n    mv a0, t0\n    ecall\n"
        );
        let stem = format!("start-{a}-{b}");
        std::fs::write(dir.join(format!("{stem}.s")), &start).expect("write start");
        for (argv, what) in [
            (
                vec![
                    "--target=riscv32",
                    "-march=rv32im",
                    "-nostdlib",
                    "-c",
                ],
                "assemble",
            ),
            (
                vec![
                    "--target=riscv32",
                    "-march=rv32im",
                    "-nostdlib",
                    "-fuse-ld=lld",
                    "-Wl,--entry,_start",
                ],
                "link",
            ),
        ] {
            let mut command = std::process::Command::new("clang");
            command.args(&argv);
            if what == "assemble" {
                command.arg(dir.join(format!("{stem}.s"))).arg("-o").arg(dir.join(format!("{stem}.o")));
            } else {
                command
                    .arg(dir.join(format!("{stem}.o")))
                    .arg(dir.join("bounded_min.o"))
                    .arg("-o")
                    .arg(dir.join(format!("prog-{stem}")));
            }
            let output = command.output().expect("run clang");
            assert!(
                output.status.success(),
                "clang {what} must succeed for ({a}, {b}): {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let observed = std::process::Command::new("qemu-riscv32")
            .arg(dir.join(format!("prog-{stem}")))
            .output()
            .expect("run qemu-riscv32");
        // The exit code IS the observed value: the freestanding image
        // returns bounded_min(a, b) via the exit syscall. Signal death
        // (no code) is the only harness-level failure here.
        let code = observed.status.code().unwrap_or_else(|| {
            panic!(
                "qemu-riscv32 died without an exit code for ({a}, {b}): stderr={}",
                String::from_utf8_lossy(&observed.stderr)
            )
        });
        let expected = reference_min(&dir, a, b);
        assert_eq!(
            code as i64, expected,
            "qemu-observed bounded_min({a}, {b}) must equal the MNCS reference"
        );
    }
}
