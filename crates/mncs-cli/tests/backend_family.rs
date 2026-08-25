//! Backend-family conformance: every backend is exercised against the modern
//! language surface with its honest, asserted envelope. Bounded agreement is
//! evidence, not universal proof; refusals are part of the contract.

use std::process::Command;

use serde_json::Value;

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn library(name: &str) -> String {
    format!("{}/../../library/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn run_experiment(source: &str, backend: &str, corpus: &str) -> Value {
    let output = binary()
        .args([
            "experiment",
            "run",
            source,
            "--backend",
            backend,
            "--corpus",
            corpus,
        ])
        .output()
        .expect("run experiment");
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("experiment JSON for {backend}: {error}"))
}

/// Count corpus cases whose declared expectations were met, whether those
/// expectations are return values (`expectation_met`) or runtime-failure
/// statuses (`status_met`).
fn cases_met(result: &Value) -> usize {
    result["cases"]
        .as_array()
        .map(|cases| {
            cases
                .iter()
                .filter(|case_| case_["expectation_met"] == true || case_["status_met"] == true)
                .count()
        })
        .unwrap_or(0)
}

fn refusal_codes(result: &Value) -> Vec<String> {
    result["diagnostics"]
        .as_array()
        .map(|diagnostics| {
            diagnostics
                .iter()
                .filter_map(|diag| diag["code"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// The checked division/modulo corpus across every executable scalar backend:
/// identical bounded observations everywhere, including runtime-failure
/// classification for division by zero and MIN / -1. The Cranelift case is
/// load-bearing for process safety: an unguarded native `sdiv` would kill the
/// CLI under JIT instead of returning structured JSON.
#[test]
fn checked_division_agrees_across_executable_backends() {
    let source = example("execution/backend-conformance/div-probe.mncs");
    let corpus = example("execution/backend-conformance/div-corpus.json");
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let result = run_experiment(&source, backend, &corpus);
        // Overall stays UNKNOWN because the fixture uses checked arithmetic;
        // every behavioral case must still be met by every backend.
        assert_eq!(
            result["status"], "UNKNOWN",
            "{backend}: {:#?}",
            result["cases"]
        );
        assert_eq!(cases_met(&result), 7, "{backend}");
    }
}

/// Unsigned division agrees with the reference executor on every executable
/// scalar backend: the high-bit dividend distinguishes genuine unsigned
/// division from signed reinterpretation (0x1/2u = 0 vs 0xFFFFFFFF/2u =
/// 2147483647), and a zero divisor fails identically everywhere.
#[test]
fn unsigned_division_agrees_across_executable_backends() {
    let source = example("source/profile06-unsigned-division.mncs");
    let corpus = example("execution/backend-conformance/unsigned-div-corpus.json");
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let result = run_experiment(&source, backend, &corpus);
        assert_eq!(cases_met(&result), 3, "{backend}: {:#?}", result["cases"]);
    }
}

/// Strict boolean operators agree across all five executable backends.
#[test]
fn strict_booleans_agree_across_executable_backends() {
    let source = example("source/profile06-boolean-operators.mncs");
    let corpus = example("execution/profile06-boolean-operators-corpus.json");
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let result = run_experiment(&source, backend, &corpus);
        assert_eq!(
            result["status"], "PASS",
            "{backend}: {:#?}",
            result["cases"]
        );
        assert_eq!(cases_met(&result), 9, "{backend}");
    }
}

/// The core library runs end to end on bytecode and WASM (records, payload
/// sums); C11/LLVM/Cranelift refuse composite signatures explicitly instead
/// of mis-typing them.
#[test]
fn core_status_module_envelope_per_backend() {
    let source = library("core/status.mncs");
    let corpus = example("execution/library-core-status-corpus.json");

    for backend in ["mncs-research-bytecode", "mncs-portable-wasm-mvp"] {
        let result = run_experiment(&source, backend, &corpus);
        assert_eq!(result["status"], "PASS", "{backend}");
        assert_eq!(cases_met(&result), 30, "{backend}");
    }

    for backend in ["mncs-c11", "mncs-llvm-ir"] {
        let result = run_experiment(&source, backend, &corpus);
        assert_ne!(
            result["status"], "PASS",
            "{backend} must not claim composite support it does not have"
        );
        let codes = refusal_codes(&result);
        assert!(
            codes.iter().any(|code| code.starts_with("CG")),
            "{backend}: expected a structured capability refusal, got {codes:?}"
        );
    }
}

/// The Result payload-sum module executes identically on bytecode and WASM.
#[test]
fn core_result_payload_sums_on_bytecode_and_wasm() {
    let source = library("core/result.mncs");
    let corpus = example("execution/library-core-result-corpus.json");
    for backend in ["mncs-research-bytecode", "mncs-portable-wasm-mvp"] {
        let result = run_experiment(&source, backend, &corpus);
        // UNKNOWN only from honest checked-arithmetic obligations.
        assert_eq!(result["status"], "UNKNOWN", "{backend}");
        assert_eq!(cases_met(&result), 8, "{backend}");
    }
}

// --- External targets -------------------------------------------------------

fn llc_available() -> bool {
    Command::new("llc")
        .arg("--version")
        .output()
        .is_some_and(|output| output.status.success())
}

/// Compile one source module for one external target. Returns the parsed JSON
/// plus the raw artifact bytes when the backend produced one.
fn compile_for_target(source: &str, target: &str) -> (Value, Option<Vec<u8>>) {
    let output = binary()
        .args(["compile", source, "--emit", "backend", "--target", target])
        .output()
        .expect("compile for external target");
    assert!(
        output.status.success(),
        "{target}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{target}: compilation JSON: {error}",));
    // Semantic obligation state and lowering success are independent axes:
    // unresolved program obligations must not be read as backend failure,
    // and backend success must not be read as fully verified semantics.
    match result["status"].as_str() {
        Some("completed") | Some("completed_with_unresolved_obligations") => {}
        other => panic!("{target}: unexpected top-level status {other:?}"),
    }
    let artifact = &result["emissions"]["backend"];
    if artifact.is_null() {
        return (result, None);
    }
    assert_eq!(
        artifact["bytes_sha256"].as_str().map(str::len),
        Some(64),
        "{target}: artifact lacks a content digest"
    );
    let bytes = hex_decode(artifact["bytes_hex"].as_str().unwrap_or_default());
    assert!(!bytes.is_empty(), "{target}: empty artifact bytes");
    (result, Some(bytes))
}

fn hex_decode(hex: &str) -> Vec<u8> {
    (0..hex.len() / 2)
        .map(|index| {
            u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).expect("valid hex byte")
        })
        .collect()
}

/// ELF structural expectations per object target, checked independently of
/// the compiler's own validation: magic, class, endianness, ET_REL, and
/// machine identifier.
fn validate_elf(bytes: &[u8], expected_class: u8, expected_machine: u16) {
    assert!(bytes.len() > 64, "ELF too short for a header");
    assert_eq!(&bytes[..4], &[0x7f, b'E', b'L', b'F'], "ELF magic");
    assert_eq!(bytes[4], expected_class, "ELF class");
    assert_eq!(bytes[5], 1, "ELF data encoding must be little-endian");
    let read_u16 = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    assert_eq!(read_u16(16), 1, "ELF type must be ET_REL");
    assert_eq!(read_u16(18), expected_machine, "ELF e_machine");
}

/// RISC-V: a genuine RV32IM ELF object is produced through the LLVM toolchain
/// where available; its absence is an explicit environment refusal, never a
/// fabricated artifact.
#[test]
fn riscv32_produces_genuine_elf_artifact() {
    let (result, bytes) = compile_for_target(
        &example("source/profile06-boolean-operators.mncs"),
        "mncs-riscv32",
    );
    let Some(bytes) = bytes else {
        // No artifact: the refusal must name the missing toolchain or a
        // precise lowering failure, and no execution may be claimed.
        let codes = refusal_codes(&result);
        assert!(
            codes.iter().any(|code| code.starts_with("CGX4")),
            "expected a coded external-target refusal, got {codes:?}"
        );
        return;
    };
    validate_elf(&bytes, 1, 243);
    assert_eq!(
        result["emissions"]["backend"]["artifact_kind"],
        "elf_object_riscv32"
    );
    let exports = result["emissions"]["backend"]["exports"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(!exports.is_empty(), "RISC-V artifact declares no exports");
}

/// eBPF: a genuine little-endian BPF ELF object (EM_BPF = 247) is produced
/// where the toolchain exists.
#[test]
fn ebpf_produces_genuine_elf_artifact() {
    let (result, bytes) = compile_for_target(
        &example("source/profile06-boolean-operators.mncs"),
        "mncs-ebpf",
    );
    let Some(bytes) = bytes else {
        let codes = refusal_codes(&result);
        assert!(
            codes.iter().any(|code| code.starts_with("CGX4")),
            "expected a coded external-target refusal, got {codes:?}"
        );
        return;
    };
    validate_elf(&bytes, 2, 247);
    assert_eq!(
        result["emissions"]["backend"]["artifact_kind"],
        "elf_object_ebpf"
    );
}

/// PTX: meaningful module syntax — version, shader-model target matching the
/// declared contract, 64-bit addressing, and visible definitions for every
/// declared export.
#[test]
fn ptx64_produces_genuine_ptx_module() {
    let (result, bytes) = compile_for_target(
        &example("source/profile06-boolean-operators.mncs"),
        "mncs-ptx64",
    );
    let Some(bytes) = bytes else {
        let codes = refusal_codes(&result);
        assert!(
            codes.iter().any(|code| code.starts_with("CGX4")),
            "expected a coded external-target refusal, got {codes:?}"
        );
        return;
    };
    let text = String::from_utf8(bytes).expect("PTX is UTF-8 text");
    assert!(
        text.lines()
            .any(|line| line.trim_start().starts_with(".version ")),
        "PTX lacks .version"
    );
    let target_line = text
        .lines()
        .find_map(|line| line.trim().strip_prefix(".target "))
        .expect("PTX lacks .target");
    assert!(
        target_line.starts_with("sm_"),
        "unexpected PTX target {target_line}"
    );
    assert!(
        text.contains(".address_size 64"),
        "PTX lacks 64-bit addressing"
    );
    for export in result["emissions"]["backend"]["exports"]
        .as_array()
        .expect("export list")
    {
        let name = export.as_str().expect("export name");
        assert!(
            text.contains(&format!(".visible .func {name}(")),
            "PTX lacks visible definition for {name}"
        );
    }
}

/// eBPF cannot lower signed division upstream; where the LLVM toolchain is
/// present the restriction must surface as a structured target-realization
/// refusal with no artifact — never a crash, silent miscompilation, or
/// fabricated success. Without the toolchain the same module refuses as an
/// environment limitation instead.
#[test]
fn ebpf_refuses_signed_division_precisely() {
    let output = binary()
        .args([
            "compile",
            &example("execution/backend-conformance/div-probe.mncs"),
            "--emit",
            "backend",
            "--target",
            "mncs-ebpf",
        ])
        .output()
        .expect("compile for eBPF");
    // Compilation itself completes; the restriction is reported
    // structurally rather than as a process failure.
    let result: Value = serde_json::from_slice(&output.stdout).expect("compilation JSON");
    assert_eq!(result["status"], "completed_with_unresolved_obligations");
    assert!(
        result["emissions"]["backend"].is_null(),
        "eBPF must not emit an artifact for signed division modules"
    );
    let diagnostics = result["diagnostics"].as_array().expect("diagnostics");
    let expected_code = if llc_available() { "CGX410" } else { "CGX401" };
    let refused = diagnostics.iter().any(|diag| diag["code"] == expected_code);
    assert!(refused, "expected {expected_code}, got {diagnostics:#?}");
}

/// Every external target refuses host execution honestly: no emulator or GPU
/// runtime is claimed where none exists. Cases report `unsupported` rather
/// than fabricated returns.
#[test]
fn external_targets_refuse_execution_honestly() {
    let source = example("source/profile06-boolean-operators.mncs");
    let corpus = example("execution/profile06-boolean-operators-corpus.json");
    for target in ["mncs-riscv32", "mncs-ebpf", "mncs-ptx64"] {
        let result = run_experiment(&source, target, &corpus);
        assert_eq!(result["status"], "FAIL", "{target} must not claim success");
        let cases = result["cases"].as_array().expect("cases");
        assert!(
            !cases.is_empty() && cases.iter().all(|case| case["status"] == "unsupported"),
            "{target}: every case must refuse as unsupported: {cases:#?}"
        );
    }
}
