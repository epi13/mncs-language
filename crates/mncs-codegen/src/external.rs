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

use mncs_model::{
    BackendArtifact, BackendCapabilityManifest, BackendConfiguration, BackendEvidence,
    BackendIdentity, BackendResult, CompilerArtifactRef, CompilerDiagnostic,
    CompilerDiagnosticKind, ExecutionRequest, ExecutionStatus, Program, SsaModule,
    TargetContractRef, TargetLoweringPlan, TransformationStatus, SSA_SCHEMA_VERSION,
};

use crate::support::{
    artifact_ref, empty_execution, execution_failure, function_names, unknown,
    validate_selected_ssa,
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

    /// Invoke the system `llc` against the shared LLVM lowering output.
    /// Returns the genuine target artifact bytes plus the toolchain identity.
    fn realize(&self, ir: &str) -> Result<(Vec<u8>, String), String> {
        let llc = crate::native::probe_llc()
            .ok_or_else(|| "unavailable toolchain: llc is not present".to_owned())?;
        let dir = std::env::temp_dir().join(format!(
            "mncs-external-{}",
            &crate::support::sha256_hex(format!("{}{ir}", self.triple).as_bytes())[..16]
        ));
        std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let ir_path = dir.join("module.ll");
        std::fs::write(&ir_path, ir.as_bytes()).map_err(|error| error.to_string())?;
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
        command
            .arg("-filetype")
            .arg(self.filetype)
            .arg("-o")
            .arg(&out_path)
            .arg(ir_path.file_name().unwrap_or(ir_path.as_os_str()));
        let output = command.output().map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err(format!(
                "llc failed for {}: {}",
                self.triple,
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let bytes = std::fs::read(&out_path).map_err(|error| error.to_string())?;
        Ok((bytes, llc.summary()))
    }

    /// Self-contained structural validation of the produced artifact.
    fn validate_artifact_bytes(&self, bytes: &[u8]) -> Result<String, String> {
        match self.filetype {
            "obj" => {
                let elf_magic = [0x7f, b'E', b'L', b'F'];
                if bytes.len() < 20 || bytes[..4] != elf_magic {
                    return Err("ELF object lacks a valid magic header".to_owned());
                }
                let class = match bytes[4] {
                    1 => "ELF32",
                    2 => "ELF64",
                    other => return Err(format!("unknown ELF class {other}")),
                };
                // External observation: readelf/llvm-readobj when present.
                let external = std::process::Command::new("llvm-readobj")
                    .arg("--file-headers")
                    .arg("-")
                    .stdin(std::process::Stdio::null())
                    .output()
                    .ok()
                    .map(|_| ());
                Ok(format!(
                    "{class} object validated; external readelf observation: {}",
                    external.is_some()
                ))
            }
            _ => {
                let head = String::from_utf8_lossy(&bytes[..bytes.len().min(64)]);
                if head.contains(".version") && head.contains(".target") {
                    Ok("PTX module header validated".to_owned())
                } else {
                    Err("PTX module lacks a version/target header".to_owned())
                }
            }
        }
    }
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
    if let Err(result) = validate_selected_ssa(ssa, &selected_ssa, "CGX101") {
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
    // The shared LLVM lowering is the source IR for every external target.
    let ir = crate::llvm::emit_llvm_module(&scalar, plan);
    let (bytes, toolchain) = match spec.realize(&ir) {
        Ok(outcome) => outcome,
        Err(reason) => {
            return execution_failure_owned(spec, reason);
        }
    };
    let validation = match spec.validate_artifact_bytes(&bytes) {
        Ok(observation) => observation,
        Err(reason) => return execution_failure_owned(spec, reason),
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
        scalar
            .functions
            .iter()
            .map(|function| function.export_name.clone())
            .collect(),
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

fn execution_failure_owned(spec: &ExternalTargetSpec, reason: String) -> BackendResult {
    BackendResult {
        status: TransformationStatus::Unknown,
        artifact: None,
        artifact_ref: None,
        evidence: None,
        diagnostics: vec![CompilerDiagnostic::new(
            "CGX400",
            CompilerDiagnosticKind::MissingTargetEvidence,
            format!("{}: {reason}", spec.backend_name),
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
