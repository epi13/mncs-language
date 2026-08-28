//! Machine-readable backend family matrix generated from registered adapters.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{backend_adapter, backend_names};

pub const BACKEND_FAMILY_MATRIX_SCHEMA_VERSION: &str = "0.1";
pub const BACKEND_FAMILY_MATRIX_INTERPRETATION: &str =
    "capability_envelope_from_registered_adapters_not_universal_equivalence";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendMatrixRow {
    pub backend: String,
    pub version: String,
    pub realization_profile: String,
    pub artifact_kinds: BTreeSet<String>,
    pub supported_machine_intents: BTreeSet<String>,
    pub unsupported_operations: BTreeSet<String>,
    pub required_target_facts: BTreeSet<String>,
    pub runtime_requirements: BTreeSet<String>,
    pub embedded_execution: bool,
    pub external_execution: bool,
    pub target_families: BTreeSet<String>,
    pub supported_source_profiles: Vec<String>,
    pub cre1: String,
    pub cre2: String,
    pub cre3: String,
    pub unresolved_obligations: Vec<String>,
    pub known_limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendFamilyMatrix {
    pub schema_version: String,
    pub interpretation: String,
    pub planned_unimplemented: Vec<String>,
    pub backends: Vec<BackendMatrixRow>,
}

pub fn backend_family_matrix() -> BackendFamilyMatrix {
    let mut backends = Vec::new();
    for name in backend_names() {
        let Some(adapter) = backend_adapter(name) else {
            continue;
        };
        let capabilities = adapter.capabilities();
        let embedded = capabilities
            .runtime_requirements
            .iter()
            .any(|requirement| requirement.contains("interpreter") || requirement.contains("jit"));
        let external = capabilities
            .runtime_requirements
            .iter()
            .any(|requirement| requirement.contains("external") || requirement.contains("clang"));
        backends.push(BackendMatrixRow {
            backend: capabilities.backend.name.clone(),
            version: capabilities.backend.version.clone(),
            realization_profile: capabilities.realization_profile.clone(),
            artifact_kinds: capabilities.artifact_kinds.clone(),
            supported_machine_intents: capabilities.supported_machine_intents.clone(),
            unsupported_operations: capabilities.unsupported_operations.clone(),
            required_target_facts: capabilities.required_target_facts.clone(),
            runtime_requirements: capabilities.runtime_requirements.clone(),
            embedded_execution: embedded,
            external_execution: external,
            target_families: capabilities.target_families.clone(),
            supported_source_profiles: vec![
                "0.1".to_owned(),
                "0.2".to_owned(),
                "0.3".to_owned(),
                "0.4".to_owned(),
                "0.5".to_owned(),
            ],
            cre1: "untested_in_this_matrix".to_owned(),
            cre2: "untested_in_this_matrix".to_owned(),
            cre3: "untested_in_this_matrix".to_owned(),
            unresolved_obligations: vec![
                "exact instruction cost remains UNKNOWN".to_owned(),
                "cross-backend agreement is bounded observation".to_owned(),
            ],
            known_limitations: capabilities
                .unsupported_operations
                .iter()
                .cloned()
                .collect(),
        });
    }
    BackendFamilyMatrix {
        schema_version: BACKEND_FAMILY_MATRIX_SCHEMA_VERSION.to_owned(),
        interpretation: BACKEND_FAMILY_MATRIX_INTERPRETATION.to_owned(),
        // Implemented this campaign: riscv32, ebpf, ptx64 (external LLVM
        // realizations with artifact validation). Still planned:
        // SPIR-V codegen, host execution for the external families
        // (emulators/kernel verifier/GPU runtime), bare-metal bring-up,
        // Native SIMD selection and nested composite sequence elements.
        planned_unimplemented: vec![
            "spir-v / gpu compute codegen".to_owned(),
            "riscv execution via emulator on hosts that lack one".to_owned(),
            "eBPF kernel-verifier observations on privileged hosts".to_owned(),
            "ptx GPU execution where a driver exists".to_owned(),
            "additional vm/bytecode formats".to_owned(),
            "bare-metal target bring-up (no OS runtime)".to_owned(),
        ],
        backends,
    }
}

pub fn with_experiment_status(
    mut matrix: BackendFamilyMatrix,
    backend: &str,
    cre1: &str,
    cre2: &str,
    cre3: &str,
) -> BackendFamilyMatrix {
    if let Some(row) = matrix
        .backends
        .iter_mut()
        .find(|row| row.backend == backend)
    {
        row.cre1 = cre1.to_owned();
        row.cre2 = cre2.to_owned();
        row.cre3 = cre3.to_owned();
    }
    matrix
}
