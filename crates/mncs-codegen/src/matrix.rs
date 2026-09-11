//! Machine-readable backend family matrix generated from registered adapters.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{backend_adapter, backend_names};

pub const BACKEND_FAMILY_MATRIX_SCHEMA_VERSION: &str = "0.1";
pub const BACKEND_FAMILY_MATRIX_INTERPRETATION: &str =
    "capability_envelope_from_registered_adapters_not_universal_equivalence";

/// Realization strength of one source profile on one backend.
///
/// - `realized`: the profile's constructs lower and execute (embedded) or
///   produce validated artifacts (external) with no known gaps.
/// - `partial`: lowerable, with explicit `unsupported_capabilities` gaps.
/// - `artifact_only`: compiles to an artifact; execution is out of scope
///   (external families) or refused.
/// - `unsupported`: outside the backend's intent envelope; do not route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendProfileSupport {
    pub profile: String,
    pub status: String,
    pub unsupported_capabilities: Vec<String>,
}

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
    /// Lowerable envelope: profiles with status `realized`, `partial`, or
    /// `artifact_only`. "Supported" means the backend can be asked to
    /// lower the profile — NOT that every capability executes. Consult
    /// `profile_support` for realization strength.
    pub supported_source_profiles: Vec<String>,
    pub profile_support: Vec<BackendProfileSupport>,
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

fn support(profile: &str, status: &str, gaps: &[&str]) -> BackendProfileSupport {
    BackendProfileSupport {
        profile: profile.to_owned(),
        status: status.to_owned(),
        unsupported_capabilities: gaps.iter().map(|gap| (*gap).to_owned()).collect(),
    }
}

fn realized(profile: &str) -> BackendProfileSupport {
    support(profile, "realized", &[])
}

/// Evidence-backed per-backend profile envelope.
///
/// Curated from lowering refusals and executed conformance, not from
/// intent-set vibes: `host_write` realization is bytecode-only (lowering
/// refusal on the other four executables), filesystem observation is
/// bytecode-only (explicit refusal tests on wasm/c11/llvm/cranelift),
/// filesystem mutation is bytecode-only (whole-program refusal of the
/// all-effectful mutation module on all four compiled backends), and
/// external families have artifact evidence only through 0.6
/// (`bounded-min.mncs` at 0.2, `profile06-boolean-operators.mncs` at
/// 0.6). Unknown backend names fail closed to all-`unsupported`.
///
/// The matrix conformance test requires every registry profile to appear
/// here, so a new profile cannot land without updating this envelope.
pub fn profile_support_for(backend_name: &str) -> Vec<BackendProfileSupport> {
    // Profiles 0.1-0.7: core through bounded sequences; executed on all
    // five backends by long-standing corpora.
    let core: Vec<BackendProfileSupport> = ["0.1", "0.2", "0.3", "0.4", "0.5", "0.6", "0.7"]
        .iter()
        .map(|profile| realized(profile))
        .collect();
    // Profiles 0.9-0.11: namespaces, generics, nested iteration —
    // compile-time plus constructs covered by five-backend corpora.
    let mid: Vec<BackendProfileSupport> = ["0.9", "0.10", "0.11"]
        .iter()
        .map(|profile| realized(profile))
        .collect();
    match backend_name {
        "mncs-research-bytecode" => core
            .into_iter()
            .chain([realized("0.8")])
            .chain(mid)
            .chain([
                realized("0.12"),
                support("0.13", "partial", &["structural_recursion"]),
                // Buffer pipelines execute end to end: span-copy, view
                // narrowing, and checked-index corpora all agree on all
                // five executable backends.
                realized("0.14"),
                // Static view widening is elaboration-only (no new
                // runtime operation; descriptors are identical), so it
                // executes wherever 0.14 does: the widen corpus agrees
                // on all five executable backends.
                realized("0.15"),
                // Durable filesystem mutation executes end to end: the
                // fixture-anchored mutation corpus returns on every case
                // with overall status PASS (independent observed replay
                // agrees layer-by-layer).
                realized("0.16"),
            ])
            .collect(),
        "mncs-portable-wasm-mvp" | "mncs-c11" | "mncs-llvm-ir" | "mncs-cranelift" => core
            .into_iter()
            .chain([support("0.8", "partial", &["host_write_realization"])])
            .chain(mid)
            .chain([
                support("0.12", "partial", &["fs_observation"]),
                support(
                    "0.13",
                    "partial",
                    &["fs_observation", "structural_recursion"],
                ),
                realized("0.14"),
                // Same elaboration-only argument as the research path:
                // the widen corpus agrees on all four compiled backends.
                realized("0.15"),
                // Filesystem mutation is bytecode-only: the all-effectful
                // mutation module refuses whole-program with explicit
                // host-call diagnostics (per-entrypoint admission keeps
                // pure neighbors realizable), so compiled backends stay
                // partial with the mutation gap named.
                support("0.16", "partial", &["fs_mutation"]),
            ])
            .collect(),
        "mncs-riscv32" | "mncs-ebpf" | "mncs-ptx64" => ["0.1", "0.2", "0.3", "0.4", "0.5", "0.6"]
            .iter()
            .map(|profile| support(profile, "artifact_only", &[]))
            .chain(
                [
                    "0.7", "0.8", "0.9", "0.10", "0.11", "0.12", "0.13", "0.14", "0.15",
                    "0.16",
                ]
                .iter()
                .map(|profile| {
                    support(
                        profile,
                        "unsupported",
                        &["outside_external_intent_envelope"],
                    )
                }),
            )
            .collect(),
        _ => mncs_syntax::SOURCE_PROFILE_REGISTRY
            .iter()
            .map(|record| support(record.version, "unsupported", &["unknown_backend"]))
            .collect(),
    }
}

/// Profiles the backend can be asked to lower: `realized`, `partial`, or
/// `artifact_only`. Derived from the same table so the two fields cannot
/// drift.
fn lowerable_profiles(support: &[BackendProfileSupport]) -> Vec<String> {
    support
        .iter()
        .filter(|entry| entry.status != "unsupported")
        .map(|entry| entry.profile.clone())
        .collect()
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
        let support = profile_support_for(&capabilities.backend.name);
        let lowerable = lowerable_profiles(&support);
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
            supported_source_profiles: lowerable,
            profile_support: support,
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
        // and native SIMD selection. Nested composite sequence elements
        // and logical `[bool; N]` windows are now realized.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn registry_versions() -> Vec<String> {
        mncs_syntax::SOURCE_PROFILE_REGISTRY
            .iter()
            .map(|record| record.version.to_owned())
            .collect()
    }

    /// Every registry profile must appear in every backend's envelope, so
    /// a new profile cannot land without updating backend metadata.
    #[test]
    fn profile_envelope_covers_every_registry_profile() {
        let versions = registry_versions();
        for name in crate::backend_names() {
            let support = profile_support_for(name);
            let listed: Vec<String> = support.iter().map(|entry| entry.profile.clone()).collect();
            assert_eq!(
                listed, versions,
                "{name}: profile envelope must track the registry exactly"
            );
            for entry in &support {
                assert!(
                    ["realized", "partial", "artifact_only", "unsupported"]
                        .contains(&entry.status.as_str()),
                    "{name} {}: status outside the vocabulary",
                    entry.profile
                );
                if entry.status == "partial" || entry.status == "unsupported" {
                    assert!(
                        !entry.unsupported_capabilities.is_empty(),
                        "{name} {}: a non-realized status must name its gaps",
                        entry.profile
                    );
                }
            }
        }
    }

    /// `supported_source_profiles` is derived from the same table, so the
    /// two fields cannot drift.
    #[test]
    fn lowerable_envelope_matches_profile_support() {
        let matrix = backend_family_matrix();
        assert_eq!(matrix.backends.len(), crate::backend_names().len());
        for row in &matrix.backends {
            let derived = lowerable_profiles(&row.profile_support);
            assert_eq!(
                row.supported_source_profiles, derived,
                "{}: lowerable envelope drifted from profile_support",
                row.backend
            );
        }
    }

    /// External families never execute: they must not claim `realized`.
    #[test]
    fn external_backends_never_claim_realized() {
        for name in ["mncs-riscv32", "mncs-ebpf", "mncs-ptx64"] {
            for entry in profile_support_for(name) {
                assert_ne!(
                    entry.status, "realized",
                    "{name} {}: external families are artifact-only at best",
                    entry.profile
                );
            }
        }
    }

    /// Unknown backends fail closed instead of inheriting an envelope.
    #[test]
    fn unknown_backends_fail_closed() {
        for entry in profile_support_for("mncs-does-not-exist") {
            assert_eq!(entry.status, "unsupported");
        }
    }
}
