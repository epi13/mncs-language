//! External-target realizations through the system LLVM toolchain.
//!
//! These adapters reuse the exact LLVM scalar lowering and then lower the
//! resulting IR to a genuine target artifact via `llc` (RISC-V ELF, eBPF
//! ELF, NVIDIA PTX). External tools are observations, not MNCS semantics;
//! their identity is recorded with every artifact. Execution on this host is
//! refused honestly where no emulator/runtime exists: the refusal names the
//! missing environment capability instead of pretending the backend does not
//! exist or fabricating an execution claim.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use mncs_model::{
    BackendArtifact, BackendCapabilityManifest, BackendConfiguration, BackendEvidence,
    BackendIdentity, BackendResult, CompilerArtifactRef, CompilerDiagnostic,
    CompilerDiagnosticKind, ExecutionRequest, ExecutionStatus, Program, SsaModule,
    TargetContractRef, TargetLoweringPlan, TransformationStatus, SSA_SCHEMA_VERSION,
};

use crate::support::{
    artifact_ref, empty_execution, execution_failure, function_names, unknown,
    validate_realizable_ssa, validate_selected_ssa,
};
use crate::{support, BackendAdapter, BackendExecutionResult};

/// Static description of one externally realized target family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalTargetSpec {
    pub backend_name: &'static str,
    pub backend_version: &'static str,
    pub target_id: &'static str,
    /// `llc -mtriple` value.
    pub triple: &'static str,
    /// Extra `-mattr=` features appended after the triple.
    pub features: &'static [&'static str],
    /// `llc -filetype`: `obj` (binary) or `asm` (textual, e.g. PTX).
    pub filetype: &'static str,
    pub format: &'static str,
    pub artifact_kind: &'static str,
    /// Honest host-execution status recorded in capabilities and evidence.
    pub execution_note: &'static str,
}

pub const RISCV32_SPEC: ExternalTargetSpec = ExternalTargetSpec {
    backend_name: "mncs-riscv32",
    backend_version: "0.1",
    target_id: "mncs:target:riscv32-imc-0.1",
    triple: "riscv32",
    features: &["m"],
    filetype: "obj",
    format: "application/x-elf; arch=riscv32",
    artifact_kind: "elf_object_riscv32",
    execution_note:
        "artifact generation plus external ELF validation; no RISC-V emulator is installed on this host",
};

pub const EBPF_SPEC: ExternalTargetSpec = ExternalTargetSpec {
    backend_name: "mncs-ebpf",
    backend_version: "0.1",
    target_id: "mncs:target:ebpf-0.1",
    triple: "bpfel",
    features: &[],
    filetype: "obj",
    format: "application/x-elf; arch=ebpf",
    artifact_kind: "elf_object_ebpf",
    execution_note:
        "artifact generation plus external disassembly validation; no kernel verifier was run on this host",
};

pub const PTX64_SPEC: ExternalTargetSpec = ExternalTargetSpec {
    backend_name: "mncs-ptx64",
    backend_version: "0.1",
    target_id: "mncs:target:nvptx64-sm60-0.1",
    triple: "nvptx64",
    features: &[],
    filetype: "asm",
    format: "text/x-ptx; arch=nvptx64",
    artifact_kind: "ptx_module",
    execution_note:
        "PTX generation only; no ptxas or CUDA driver is installed on this host so GPU execution cannot be observed",
};

impl ExternalTargetSpec {
    fn backend(&self) -> BackendIdentity {
        BackendIdentity::new(self.backend_name, self.backend_version)
    }

    /// Process-wide monotonic nonce: together with the process id it makes
    /// every work directory unique per compilation invocation, including
    /// concurrent compilations in separate threads of the same process.
    /// Thread IDs are deliberately not used (portability and stability).
    fn invocation_nonce() -> u64 {
        static NEXT_NONCE: AtomicU64 = AtomicU64::new(0);
        NEXT_NONCE.fetch_add(1, Ordering::Relaxed)
    }

    /// Invoke the system `llc` against the shared LLVM lowering output.
    /// Returns the genuine target artifact bytes plus the toolchain identity.
    fn realize(&self, ir: &str) -> Result<(Vec<u8>, String), RealizeError> {
        let llc = crate::native::probe_llc().ok_or_else(|| RealizeError {
            code: "CGX401",
            message: "unavailable toolchain: llc is not present on this host".to_owned(),
        })?;
        // The work directory is unique per compilation invocation: concurrent
        // compilations of the same program and target, whether in separate
        // processes or separate threads of one process, must never share
        // module.ll/module.o, and cleanup must never remove a live directory.
        let dir = std::env::temp_dir().join(format!(
            "mncs-external-{}-{}-{}",
            &crate::support::sha256_hex(format!("{}{ir}", self.triple).as_bytes())[..16],
            std::process::id(),
            Self::invocation_nonce()
        ));
        std::fs::create_dir_all(&dir).map_err(|error| RealizeError {
            code: "CGX402",
            message: format!("unable to create work directory: {error}"),
        })?;
        let ir_path = dir.join("module.ll");
        std::fs::write(&ir_path, ir.as_bytes()).map_err(|error| RealizeError {
            code: "CGX402",
            message: format!("unable to write LLVM IR input: {error}"),
        })?;
        let out_path = dir.join(if self.filetype == "obj" {
            "module.o"
        } else {
            "module.ptx"
        });
        let mut command = std::process::Command::new(&llc.path);
        command.current_dir(&dir);
        command.arg("-mtriple").arg(self.triple);
        if !self.features.is_empty() {
            command.arg("-mattr").arg(self.features.join(","));
        }
        if self.triple == "nvptx64" {
            // The declared target contract is sm_60; realize exactly that
            // shader model instead of the toolchain default.
            command.arg("-mcpu=sm_60");
        }
        command
            .arg("-filetype")
            .arg(self.filetype)
            .arg("-o")
            .arg(&out_path)
            .arg(ir_path.file_name().unwrap_or(ir_path.as_os_str()));
        let output = command.output().map_err(|error| RealizeError {
            code: "CGX400",
            message: format!("llc could not be executed: {error}"),
        })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(
                if stderr.contains("unsupported") || stderr.contains("not supported") {
                    // A precise target restriction, not a broken toolchain:
                    // e.g. eBPF's upstream inability to lower signed division.
                    RealizeError {
                        code: "CGX410",
                        message: format!("target restriction for {}: {stderr}", self.triple),
                    }
                } else {
                    RealizeError {
                        code: "CGX400",
                        message: format!("llc failed for {}: {stderr}", self.triple),
                    }
                },
            );
        }
        let bytes = std::fs::read(&out_path).map_err(|error| RealizeError {
            code: "CGX402",
            message: format!("unable to read produced artifact: {error}"),
        })?;
        // Best-effort cleanup: the directory is unique to this invocation,
        // so removing it cannot disturb a concurrent compilation.
        let _ = std::fs::remove_dir_all(&dir);
        Ok((bytes, llc.summary()))
    }

    /// Self-contained structural validation of the produced artifact. The
    /// emitted bytes are parsed directly so the observation is deterministic
    /// and does not depend on optional host tools.
    fn validate_artifact_bytes(&self, bytes: &[u8], exports: &[String]) -> Result<String, String> {
        match self.filetype {
            "obj" => validate_elf_object(bytes, self.triple, exports),
            _ => validate_ptx_module(bytes, exports),
        }
    }
}

/// A lowering refusal with its own diagnostic identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizeError {
    pub code: &'static str,
    pub message: String,
}

fn read_u16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn read_u64(bytes: &[u8], at: usize) -> u64 {
    u64::from_le_bytes([
        bytes[at],
        bytes[at + 1],
        bytes[at + 2],
        bytes[at + 3],
        bytes[at + 4],
        bytes[at + 5],
        bytes[at + 6],
        bytes[at + 7],
    ])
}

fn c_str_at(bytes: &[u8], start: usize, end: usize) -> Option<&str> {
    let region = bytes.get(start..end.min(bytes.len()))?;
    let stop = region
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(region.len());
    std::str::from_utf8(&region[..stop]).ok()
}

/// Parse and validate an ELF relocatable object against the expectations for
/// one target triple: magic, class, endianness, type, machine identifier,
/// `.text` presence, and a symbol table that defines every declared export.
fn validate_elf_object(bytes: &[u8], triple: &str, exports: &[String]) -> Result<String, String> {
    const EM_RISCV: u64 = 243;
    const EM_BPF: u64 = 247;
    const SHT_SYMTAB: u32 = 2;
    if triple != "riscv32" && triple != "bpfel" {
        return Err(format!("no ELF validation contract for triple {triple}"));
    }
    if bytes.len() < 64 {
        return Err("ELF object is too short to contain a header".to_owned());
    }
    if bytes[..4] != [0x7f, b'E', b'L', b'F'] {
        return Err("ELF object lacks a valid magic header".to_owned());
    }
    let (class_name, expected_class) = if triple.starts_with("riscv32") {
        ("ELF32", 1_u8)
    } else {
        ("ELF64", 2_u8)
    };
    if bytes[4] != expected_class {
        return Err(format!(
            "unexpected ELF class {}; {triple} requires class {expected_class} ({class_name})",
            bytes[4]
        ));
    }
    if bytes[5] != 1 {
        return Err(format!(
            "unexpected ELF data encoding {} (big-endian); {triple} artifacts are little-endian",
            bytes[5]
        ));
    }
    let e_type = read_u16(bytes, 16);
    if e_type != 1 {
        return Err(format!(
            "ELF type {e_type} is not ET_REL (relocatable object)"
        ));
    }
    let e_machine = read_u16(bytes, 18);
    let expected_machine = if triple == "riscv32" {
        EM_RISCV
    } else {
        EM_BPF
    };
    if u64::from(e_machine) != expected_machine {
        return Err(format!(
            "ELF e_machine {e_machine} does not match expected {expected_machine} for {triple}"
        ));
    }
    let is_32 = expected_class == 1;
    let shoff = if is_32 {
        u64::from(read_u32(bytes, 32))
    } else {
        read_u64(bytes, 40)
    };
    let sh_entry_size = if is_32 { 40 } else { 64 };
    let shnum = read_u16(bytes, if is_32 { 48 } else { 60 }) as u64;
    let shstrndx = read_u16(bytes, if is_32 { 50 } else { 62 }) as u64;
    if shnum == 0 || shoff == 0 {
        return Err("ELF object declares no section header table".to_owned());
    }
    if shoff + shnum * sh_entry_size as u64 > bytes.len() as u64 {
        return Err("ELF section header table exceeds file size".to_owned());
    }
    let section_header = |index: u64| -> Result<SectionHeader, String> {
        SectionHeader::parse(
            bytes,
            shoff as usize + index as usize * sh_entry_size,
            is_32,
        )
    };
    // Resolve the section-name string table through its header index.
    let shstrtab = section_header(shstrndx)?;
    if shstrtab.offset == 0 || shstrtab.offset + shstrtab.size > bytes.len() as u64 {
        return Err("ELF object lacks a readable section-name string table".to_owned());
    }
    let mut text_found = false;
    let mut symtab: Option<(u64, u64, u64, u64)> = None; // offset, size, entsize, link
    for index in 0..shnum {
        let header = section_header(index)?;
        let name = c_str_at(
            bytes,
            shstrtab.offset as usize + header.name as usize,
            (shstrtab.offset + shstrtab.size) as usize,
        )
        .unwrap_or("");
        match (name, header.section_type) {
            (".text", _) => text_found = true,
            (".symtab", SHT_SYMTAB) => {
                symtab = Some((header.offset, header.size, header.entsize, header.link));
            }
            _ => {}
        }
    }
    if !text_found {
        return Err("ELF object has no .text section".to_owned());
    }
    let Some((sym_offset, sym_size, sym_entsize, sym_link)) = symtab else {
        return Err("ELF object lacks a symbol table".to_owned());
    };
    if sym_entsize == 0 {
        return Err("ELF symbol table declares a zero entry size".to_owned());
    }
    // The string table linked from the symbol table holds symbol names.
    let strtab = section_header(sym_link)?;
    if strtab.offset == 0 || strtab.offset + strtab.size > bytes.len() as u64 {
        return Err("symbol table is not linked to a valid string table".to_owned());
    }
    // Collect defined symbol names.
    let entry_size = if is_32 { 16 } else { 24 };
    let count = sym_size / entry_size.max(1) as u64;
    let mut defined: Vec<&str> = Vec::new();
    for index in 0..count {
        let base = sym_offset as usize + index as usize * entry_size;
        if base + entry_size > bytes.len() {
            break;
        }
        let (st_name, st_shndx) = if is_32 {
            (read_u32(bytes, base), read_u16(bytes, base + 12))
        } else {
            (read_u32(bytes, base), read_u16(bytes, base + 6))
        };
        if st_shndx == 0 {
            continue; // undefined symbol reference
        }
        if let Some(name) = c_str_at(
            bytes,
            strtab.offset as usize + st_name as usize,
            (strtab.offset + strtab.size) as usize,
        ) {
            defined.push(name);
        }
    }
    for export in exports {
        if !defined.contains(&export.as_str()) {
            return format_err(format!(
                "exported symbol {export} is absent from the artifact symbol table"
            ));
        }
    }
    Ok(format!(
        "{class_name} little-endian ET_REL object; e_machine={e_machine}; \
         .text present; symbol table defines all {} declared exports",
        exports.len()
    ))
}

fn format_err(message: String) -> Result<String, String> {
    Err(message)
}

/// Parsed fields of one ELF section header, normalized to 64-bit widths.
struct SectionHeader {
    name: u32,
    section_type: u32,
    offset: u64,
    size: u64,
    link: u64,
    entsize: u64,
}

impl SectionHeader {
    fn parse(bytes: &[u8], base: usize, is_32: bool) -> Result<Self, String> {
        let entry = if is_32 { 40 } else { 64 };
        if base + entry > bytes.len() {
            return Err(format!(
                "ELF section header is truncated (base {base} + {entry} > {})",
                bytes.len()
            ));
        }
        Ok(if is_32 {
            Self {
                name: read_u32(bytes, base),
                section_type: read_u32(bytes, base + 4),
                offset: u64::from(read_u32(bytes, base + 16)),
                size: u64::from(read_u32(bytes, base + 20)),
                link: u64::from(read_u32(bytes, base + 24)),
                entsize: u64::from(read_u32(bytes, base + 36)),
            }
        } else {
            Self {
                name: read_u32(bytes, base),
                section_type: read_u32(bytes, base + 4),
                offset: read_u64(bytes, base + 24),
                size: read_u64(bytes, base + 32),
                link: u64::from(read_u32(bytes, base + 40)),
                entsize: read_u64(bytes, base + 56),
            }
        })
    }
}

/// Validate PTX module text: version and target headers plus a visible
/// function definition naming every declared export.
fn validate_ptx_module(bytes: &[u8], exports: &[String]) -> Result<String, String> {
    let text = String::from_utf8_lossy(bytes);
    let version = text
        .lines()
        .map(str::trim_start)
        .find(|line| line.starts_with(".version "))
        .and_then(|line| line.strip_prefix(".version "))
        .map(str::trim)
        .ok_or_else(|| "PTX module lacks a .version directive".to_owned())?;
    if version.is_empty() || !version.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Err(format!("PTX .version directive is malformed: {version}"));
    }
    let target = text
        .lines()
        .map(str::trim_start)
        .find(|line| line.starts_with(".target "))
        .and_then(|line| line.strip_prefix(".target "))
        .map(str::trim)
        .ok_or_else(|| "PTX module lacks a .target directive".to_owned())?;
    if !target.starts_with("sm_") {
        return Err(format!("PTX .target directive is malformed: {target}"));
    }
    if !text.contains(".address_size 64") {
        return Err("PTX module does not declare 64-bit addressing".to_owned());
    }
    for export in exports {
        let needle = format!(".visible .func {export}(");
        if !text.contains(&needle) {
            return Err(format!(
                "PTX module lacks a visible definition for export {export}"
            ));
        }
    }
    Ok(format!(
        "PTX module; version {version}; target {target}; all {} declared exports visibly defined",
        exports.len()
    ))
}

pub fn external_plan(
    spec: &ExternalTargetSpec,
    selected_ssa: CompilerArtifactRef,
) -> TargetLoweringPlan {
    let mut facts = BTreeMap::new();
    facts.insert("triple".to_owned(), spec.triple.to_owned());
    facts.insert(
        "isa".to_owned(),
        match spec.triple {
            "riscv32" => "RV32IM (base integer + multiply/divide)".to_owned(),
            "bpfel" => "eBPF (little-endian encoding)".to_owned(),
            "nvptx64" => "PTX ISA 3.2+ / sm_60 baseline".to_owned(),
            other => other.to_owned(),
        },
    );
    facts.insert(
        "abi".to_owned(),
        "module-level realization; no host process ABI is claimed for these artifacts".to_owned(),
    );
    facts.insert(
        "object-format".to_owned(),
        if spec.filetype == "obj" {
            "ELF relocatable"
        } else {
            "PTX text"
        }
        .to_owned(),
    );
    facts.insert("execution".to_owned(), spec.execution_note.to_owned());
    let target = TargetContractRef::new(spec.target_id, facts, Vec::new(), Vec::new());
    let configuration = BackendConfiguration {
        backend: spec.backend(),
        options: BTreeMap::from([
            ("llc-triple".to_owned(), spec.triple.to_owned()),
            ("llc-filetype".to_owned(), spec.filetype.to_owned()),
        ]),
        target_features: ["llvm-scalar-lowering".to_owned(), "aot-artifact".to_owned()]
            .into_iter()
            .collect(),
        linker_toolchain: None,
        assumptions: vec![spec.execution_note.to_owned()],
    };
    TargetLoweringPlan::with_explicit_facts(
        selected_ssa,
        target,
        Some(configuration),
        vec!["module-level artifact; no language-owned aggregate layout is claimed".to_owned()],
        vec![
            "no host process ABI: exports are symbols inside the artifact".to_owned(),
            "one lowered function per MNCS function via the shared LLVM scalar lowering".to_owned(),
        ],
        BTreeMap::from([
            (
                "wrapping".to_owned(),
                "target-native wrapping integer operations".to_owned(),
            ),
            (
                "checked".to_owned(),
                "explicit overflow guards inherited from the shared LLVM lowering".to_owned(),
            ),
            ("unsupported".to_owned(), "effects, runtime checks, saturating/widening intents".to_owned()),
        ]),
        BTreeMap::from([(
            "failure".to_owned(),
            "module-level artifacts carry no executable failure path; execution is an environment question".to_owned(),
        )]),
        Vec::new(),
        vec!["preserve public return and declared failure".to_owned()],
        TransformationStatus::Pass,
    )
}

pub fn lower_external(
    spec: &'static ExternalTargetSpec,
    program: &Program,
    ssa: &SsaModule,
    selected_ssa: CompilerArtifactRef,
    plan: &TargetLoweringPlan,
) -> BackendResult {
    mncs_model::record_counter("backend_lowering");
    if let Err(result) = validate_selected_ssa(ssa, &selected_ssa, "CGX101") {
        return *result;
    }
    if let Err(result) = validate_realizable_ssa(program, ssa, "CGX102") {
        return *result;
    }
    if plan.status != TransformationStatus::Pass {
        return unknown(vec![CompilerDiagnostic::new(
            "CGX202",
            CompilerDiagnosticKind::MissingTargetEvidence,
            "target lowering plan is not PASS; external adapters will not invent target facts",
        )]);
    }
    let names = function_names(program, ssa);
    let scalar = crate::scalar::lower_to_scalar(program, ssa, &names);
    if !scalar.unsupported.is_empty() || scalar.functions.is_empty() {
        let mut diagnostics = vec![CompilerDiagnostic::new(
            "CGX301",
            CompilerDiagnosticKind::UnavailableBackendCapability,
            format!(
                "selected SSA is outside the {} scalar envelope",
                spec.backend_name
            ),
        )];
        for reason in scalar.unsupported {
            diagnostics.push(CompilerDiagnostic::new(
                "CGX302",
                CompilerDiagnosticKind::UnavailableBackendCapability,
                reason,
            ));
        }
        return unknown(diagnostics);
    }
    let export_names: Vec<String> = scalar
        .functions
        .iter()
        .map(|function| function.export_name.clone())
        .collect();
    // The shared LLVM lowering is the source IR for every external target.
    let ir = crate::llvm::emit_llvm_module(&scalar, plan);
    let (bytes, toolchain) = match spec.realize(&ir) {
        Ok(outcome) => outcome,
        Err(error) => return realize_failure(spec, error),
    };
    let validation = match spec.validate_artifact_bytes(&bytes, &export_names) {
        Ok(observation) => observation,
        Err(reason) => {
            return realize_failure(
                spec,
                RealizeError {
                    code: "CGX411",
                    message: format!("artifact failed structural validation: {reason}"),
                },
            )
        }
    };
    let mut assumptions = vec![
        format!("realized through system LLVM toolchain: {toolchain}"),
        format!("external validation: {validation}"),
        spec.execution_note.to_owned(),
    ];
    assumptions.extend(plan.assumptions_introduced.clone());
    let artifact = BackendArtifact::new_with_kind(
        spec.backend(),
        selected_ssa.clone(),
        plan.target.clone(),
        spec.artifact_kind,
        spec.format,
        &bytes,
        export_names,
        assumptions.clone(),
        Vec::new(),
        vec![
            "external llc invocation".to_owned(),
            "no embedded interpreter for this target".to_owned(),
        ],
        plan.target.evidence.clone(),
        Vec::new(),
        TransformationStatus::Pass,
    )
    .with_function_value_contracts(support::function_value_contracts(program))
    .with_composite_value_contracts(support::composite_value_contracts(program));
    let artifact_ref = artifact_ref(&artifact);
    let evidence = BackendEvidence::new(
        spec.backend(),
        selected_ssa,
        artifact_ref.clone(),
        assumptions,
        plan.target.evidence.clone(),
        TransformationStatus::Pass,
    );
    BackendResult {
        status: TransformationStatus::Pass,
        artifact: Some(artifact),
        artifact_ref: Some(artifact_ref),
        evidence: Some(evidence),
        diagnostics: Vec::new(),
    }
}

/// Convert a realization refusal into the structured backend result: an
/// honest Unknown transformation state with a precise, coded diagnostic.
/// Toolchain absence (CGX401), tool failure (CGX400/402), target
/// restrictions (CGX410), and invalid artifacts (CGX411) are distinct.
fn realize_failure(spec: &ExternalTargetSpec, error: RealizeError) -> BackendResult {
    let kind = match error.code {
        "CGX410" => CompilerDiagnosticKind::UnsupportedTargetRealization,
        _ => CompilerDiagnosticKind::ExternalToolFailure,
    };
    BackendResult {
        status: TransformationStatus::Unknown,
        artifact: None,
        artifact_ref: None,
        evidence: None,
        diagnostics: vec![CompilerDiagnostic::new(
            error.code,
            kind,
            format!("{}: {}", spec.backend_name, error.message),
        )],
    }
}

/// One adapter per external target family.
#[derive(Debug, Clone, Copy)]
pub struct ExternalAdapter {
    pub spec: &'static ExternalTargetSpec,
}

impl BackendAdapter for ExternalAdapter {
    fn capabilities(&self) -> BackendCapabilityManifest {
        BackendCapabilityManifest::new(
            self.spec.backend(),
            "mncs:selected-ssa-to-external-llvm:0.1",
            [self.spec.target_id.to_owned()].into_iter().collect(),
            [SSA_SCHEMA_VERSION.to_owned()].into_iter().collect(),
            ["triple", "isa", "abi", "object-format", "execution"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            [
                "checked_integer",
                "wrapping_integer",
                "explicit_failure",
                "semantic_bounded_iteration",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            [
                "effects",
                "runtime_checks",
                "host_execution_without_emulator_or_runtime",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            [self.spec.artifact_kind.to_owned()].into_iter().collect(),
            ["external llc artifact validation".to_owned()]
                .into_iter()
                .collect(),
            ["system llvm toolchain".to_owned()].into_iter().collect(),
            ["module_level_artifact_no_process_abi".to_owned()]
                .into_iter()
                .collect(),
            self.spec.execution_note,
            false,
        )
    }

    fn target(&self) -> TargetContractRef {
        let selected = CompilerArtifactRef::new(
            mncs_model::ArtifactRepresentation::SelectedSsa,
            SSA_SCHEMA_VERSION,
            "external-target-placeholder",
        );
        external_plan(self.spec, selected).target
    }

    fn configuration(&self) -> BackendConfiguration {
        BackendConfiguration {
            backend: self.spec.backend(),
            options: BTreeMap::from([
                ("llc-triple".to_owned(), self.spec.triple.to_owned()),
                ("llc-filetype".to_owned(), self.spec.filetype.to_owned()),
            ]),
            target_features: ["llvm-scalar-lowering".to_owned(), "aot-artifact".to_owned()]
                .into_iter()
                .collect(),
            linker_toolchain: None,
            assumptions: vec![
                "the system LLVM toolchain is outside the MNCS semantic trust boundary".to_owned(),
                self.spec.execution_note.to_owned(),
            ],
        }
    }

    fn plan(&self, selected_ssa: CompilerArtifactRef) -> TargetLoweringPlan {
        external_plan(self.spec, selected_ssa)
    }

    fn lower(
        &self,
        program: &Program,
        ssa: &SsaModule,
        selected_ssa: CompilerArtifactRef,
        plan: &TargetLoweringPlan,
    ) -> BackendResult {
        lower_external(self.spec, program, ssa, selected_ssa, plan)
    }

    fn execute(
        &self,
        artifact: &mncs_model::BackendArtifact,
        request: &ExecutionRequest,
    ) -> BackendExecutionResult {
        let mut result = empty_execution(artifact, request);
        if let Err(reason) = support::backend_matches_identity(
            artifact,
            &self.spec.backend(),
            self.spec.artifact_kind,
        ) {
            return execution_failure(result, ExecutionStatus::InvalidRequest, reason);
        }
        // Honest host-execution refusal: no emulator/runtime exists here. The
        // refusal names the environment capability, not a lowering failure.
        result.status = ExecutionStatus::Unsupported;
        result.failure = Some(mncs_model::ExecutionFailure {
            identity: Some(artifact.identity.clone()),
            reason: format!(
                "{} execution is unavailable on this host: {}",
                self.spec.backend_name, self.spec.execution_note
            ),
        });
        result
    }
}

#[cfg(test)]
mod concurrency_tests {
    //! Adversarial concurrency proof for external realizations.
    //!
    //! The work-directory identity must be unique per compilation invocation:
    //! many same-IR/same-target realizations launched concurrently, in many
    //! threads of one process and across repeated stress rounds, must never
    //! collide on files, lose artifacts to cleanup races, or leak work
    //! directories. Every thread asserts byte-identical outputs.

    use std::collections::HashSet;

    use super::{RealizeError, RISCV32_SPEC};

    /// Minimal IR accepted by `llc -mtriple=riscv32 -mattr=+m -filetype=obj`.
    const PROBE_IR: &str = "define i32 @probe_min(i32 %a, i32 %b) {\nentry:\n  %c = icmp sgt i32 %a, %b\n  %m = select i1 %c, i32 %b, i32 %a\n  ret i32 %m\n}\n";

    const THREADS: usize = 16;
    const ROUNDS: usize = 5;

    fn own_work_dirs() -> HashSet<String> {
        let prefix = "mncs-external-";
        let ours = format!("-{}-", std::process::id());
        let mut found = HashSet::new();
        let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) else {
            return found;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(prefix) && name.contains(&ours) {
                found.insert(name);
            }
        }
        found
    }

    fn realize_or_toolchain_error() -> Result<(Vec<u8>, String), RealizeError> {
        RISCV32_SPEC.realize(PROBE_IR)
    }

    #[test]
    fn concurrent_same_ir_realizations_never_collide() {
        let before = own_work_dirs();
        let first = match realize_or_toolchain_error() {
            Ok((bytes, _)) => Some(bytes),
            Err(error) => {
                assert_eq!(
                    error.code, "CGX401",
                    "without llc only the structured toolchain error is acceptable"
                );
                return;
            }
        };
        let first = first.expect("realize baseline");
        assert!(!first.is_empty(), "realized artifact must not be empty");

        for _ in 0..ROUNDS {
            let mut handles = Vec::with_capacity(THREADS);
            for _ in 0..THREADS {
                handles.push(std::thread::spawn(|| realize_or_toolchain_error()));
            }
            for handle in handles {
                let (bytes, _) = handle
                    .join()
                    .expect("worker thread must not panic")
                    .expect("concurrent realization must succeed");
                assert_eq!(
                    bytes, first,
                    "concurrent same-IR realizations must be byte-identical"
                );
            }
        }

        let leaked: Vec<String> = own_work_dirs().difference(&before).cloned().collect();
        assert!(
            leaked.is_empty(),
            "no work directories may leak after cleanup: {leaked:?}"
        );
    }

    #[test]
    fn invocation_nonces_are_unique_across_threads() {
        let mut handles = Vec::with_capacity(THREADS);
        for _ in 0..THREADS {
            handles.push(std::thread::spawn(|| {
                let mut local = Vec::with_capacity(ROUNDS);
                for _ in 0..ROUNDS {
                    local.push(super::ExternalTargetSpec::invocation_nonce());
                }
                local
            }));
        }
        let mut seen = HashSet::new();
        for handle in handles {
            for nonce in handle.join().expect("nonce thread must not panic") {
                assert!(
                    seen.insert(nonce),
                    "invocation nonce duplicated: {nonce}"
                );
            }
        }
    }
}
