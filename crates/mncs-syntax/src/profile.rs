//! Authoritative source-profile registry (RFC 0036).
//!
//! Published semantic profiles are immutable historical objects: a newer
//! compiler must not silently reinterpret an older profile. This registry
//! is the single machine-readable authority for which profiles exist,
//! their status, their normative document, their predecessor relation,
//! and the resource ceilings each profile admits.
//!
//! The parser and elaborator remain the implementation of the features;
//! gates refer here for *policy* (which profile enables what, and which
//! resource ceiling applies) instead of scattering magic comparisons.

use serde::{Deserialize, Serialize};

pub const SOURCE_PROFILE_VERSION: &str = "0.1";
pub const SOURCE_PROFILE_VERSION_0_2: &str = "0.2";
pub const SOURCE_PROFILE_VERSION_0_3: &str = "0.3";
pub const SOURCE_PROFILE_VERSION_0_4: &str = "0.4";
pub const SOURCE_PROFILE_VERSION_0_5: &str = "0.5";
pub const SOURCE_PROFILE_VERSION_0_6: &str = "0.6";
pub const SOURCE_PROFILE_VERSION_0_7: &str = "0.7";
pub const SOURCE_PROFILE_VERSION_0_8: &str = "0.8";
pub const SOURCE_PROFILE_VERSION_0_9: &str = "0.9";
pub const SOURCE_PROFILE_VERSION_0_10: &str = "0.10";
pub const SOURCE_PROFILE_VERSION_0_11: &str = "0.11";
pub const SOURCE_PROFILE_VERSION_0_12: &str = "0.12";

/// Declared-but-unspecified profile identity, kept as a named constant so
/// history and diagnostics can name it. It has **no** registry record and
/// no published specification: `source_profile_supported("1.0")` is false
/// and the parser rejects it fail-closed (MNP008). Introduced accidentally
/// alongside 0.10 in commit 6636752 (RFC 0013 generics tranche); do not
/// re-admit it without a deliberate Profile 1.0 specification.
pub const SOURCE_PROFILE_VERSION_1_0: &str = "1.0";

/// True when the active source profile declares at least `version`. Profile
/// features are strictly additive, so a numeric comparison replaces the
/// per-feature version lists that previously had to name every profile.
///
/// Note the fail-closed edge: `profile_at_least("1.0", _)` is numerically
/// true, which is exactly why unsupported versions must be rejected before
/// any gate consults this function (MNP008 at parse; `identity_is_valid`
/// for envelopes).
pub fn profile_at_least(profile: &str, version: &str) -> bool {
    let parse = |value: &str| -> Option<(u64, u64)> {
        let mut parts = value.split('.');
        let major = parts.next()?.parse::<u64>().ok()?;
        let minor = parts.next()?.parse::<u64>().ok()?;
        if parts.next().is_some() {
            return None;
        }
        Some((major, minor))
    };
    match (parse(profile), parse(version)) {
        (Some(active), Some(required)) => active >= required,
        _ => false,
    }
}

/// Registry-driven support predicate: a version is supported iff it has a
/// registry record that enables parsing. `1.0` and unknown versions are
/// unsupported by construction.
pub fn source_profile_supported(version: &str) -> bool {
    source_profile_record(version).is_some_and(|record| record.supports_parsing)
}

/// Lifecycle status of a published source profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileStatus {
    /// Immutable historical profile. Implementations must preserve its
    /// acceptance/rejection behavior and semantics exactly.
    Sealed,
    /// Latest profile under active development. New semantic extensions
    /// land here with an explicit evolution relation to `predecessor`.
    Current,
}

/// One supported source profile: identity, standing, and admitted policy.
///
/// `max_sequence_bound`, `max_iteration_bound`, `max_iteration_nesting`,
/// `max_iteration_work_product`, and `max_vector_lanes` are *admitted*
/// ceilings for source declaring this profile — not the model's absolute
/// representational limits. A bound of zero means the capability does not
/// exist in that profile at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct SourceProfileRecord {
    pub version: &'static str,
    pub status: ProfileStatus,
    /// Normative specification/document path, relative to the repo root.
    pub spec_document: &'static str,
    /// Previous profile in the evolution chain (`None` for 0.1).
    pub predecessor: Option<&'static str>,
    pub supports_parsing: bool,
    pub supports_elaboration: bool,
    pub max_sequence_bound: u32,
    pub max_iteration_bound: u32,
    pub max_iteration_nesting: u32,
    pub max_iteration_work_product: u64,
    pub max_vector_lanes: u32,
    /// Capability codes introduced by this profile (headline set; the
    /// normative profile document is authoritative for details).
    pub features: &'static [&'static str],
}

/// Source Profile 0.13: consolidation/progression profile for post-0.12
/// pressure-driven extensions (RFC 0036 evolution of 0.12).
pub const SOURCE_PROFILE_VERSION_0_13: &str = "0.13";

/// Source Profile 0.14: bounded buffer-pipeline profile. Additive over
/// 0.1–0.13 with unchanged ceilings; adds bulk span copy, checked
/// view-to-view narrowing, and the checked-index discharge form.
pub const SOURCE_PROFILE_VERSION_0_14: &str = "0.14";

/// The registry is ordered oldest-first; `predecessor` links agree with
/// this order (checked by `registry_chain_is_linear`).
pub const SOURCE_PROFILE_REGISTRY: &[SourceProfileRecord] = &[
    SourceProfileRecord {
        version: "0.1",
        status: ProfileStatus::Sealed,
        spec_document: "spec/source-profile-0.1.md",
        predecessor: None,
        supports_parsing: true,
        supports_elaboration: true,
        max_sequence_bound: 0,
        max_iteration_bound: 0,
        max_iteration_nesting: 0,
        max_iteration_work_product: 0,
        max_vector_lanes: 0,
        features: &["scalar_core", "pure_functions"],
    },
    SourceProfileRecord {
        version: "0.2",
        status: ProfileStatus::Sealed,
        spec_document: "spec/source-profile-0.2.md",
        predecessor: Some("0.1"),
        supports_parsing: true,
        supports_elaboration: true,
        max_sequence_bound: 0,
        max_iteration_bound: 0,
        max_iteration_nesting: 0,
        max_iteration_work_product: 0,
        max_vector_lanes: 0,
        features: &["finite_types", "match_expressions"],
    },
    SourceProfileRecord {
        version: "0.3",
        status: ProfileStatus::Sealed,
        spec_document: "docs/source-profile-0.3.md",
        predecessor: Some("0.2"),
        supports_parsing: true,
        supports_elaboration: true,
        max_sequence_bound: 0,
        max_iteration_bound: 0,
        max_iteration_nesting: 0,
        max_iteration_work_product: 0,
        max_vector_lanes: 0,
        features: &["experiment_bootstrap", "acyclic_calls"],
    },
    SourceProfileRecord {
        version: "0.4",
        status: ProfileStatus::Sealed,
        spec_document: "docs/source-profile-0.4.md",
        predecessor: Some("0.3"),
        supports_parsing: true,
        supports_elaboration: true,
        max_sequence_bound: 0,
        max_iteration_bound: 32,
        max_iteration_nesting: 1,
        max_iteration_work_product: 32,
        max_vector_lanes: 0,
        features: &[
            "bounded_iteration_1_32",
            "scoped_iteration_identities_strict",
        ],
    },
    SourceProfileRecord {
        version: "0.5",
        status: ProfileStatus::Sealed,
        spec_document: "docs/source-profile-0.5.md",
        predecessor: Some("0.4"),
        supports_parsing: true,
        supports_elaboration: true,
        max_sequence_bound: 0,
        max_iteration_bound: 32,
        max_iteration_nesting: 1,
        max_iteration_work_product: 32,
        max_vector_lanes: 0,
        features: &["records"],
    },
    SourceProfileRecord {
        version: "0.6",
        status: ProfileStatus::Sealed,
        spec_document: "docs/source-profile-0.6.md",
        predecessor: Some("0.5"),
        supports_parsing: true,
        supports_elaboration: true,
        max_sequence_bound: 0,
        max_iteration_bound: 32,
        max_iteration_nesting: 1,
        max_iteration_work_product: 32,
        max_vector_lanes: 0,
        features: &[
            "finite_payloads",
            "explicit_arithmetic_intent",
            "bool_patterns",
        ],
    },
    SourceProfileRecord {
        version: "0.7",
        status: ProfileStatus::Sealed,
        spec_document: "docs/source-profile-0.7.md",
        predecessor: Some("0.6"),
        supports_parsing: true,
        supports_elaboration: true,
        max_sequence_bound: 64,
        max_iteration_bound: 32,
        max_iteration_nesting: 1,
        max_iteration_work_product: 64,
        max_vector_lanes: 0,
        features: &[
            "bounded_sequences_64",
            "bytes",
            "views",
            "explicit_conversion",
        ],
    },
    SourceProfileRecord {
        version: "0.8",
        status: ProfileStatus::Sealed,
        spec_document: "docs/source-profile-0.8.md",
        predecessor: Some("0.7"),
        supports_parsing: true,
        supports_elaboration: true,
        max_sequence_bound: 64,
        max_iteration_bound: 32,
        max_iteration_nesting: 1,
        max_iteration_work_product: 64,
        max_vector_lanes: 64,
        features: &["semantic_vectors", "strict_select", "host_effects"],
    },
    SourceProfileRecord {
        version: "0.9",
        status: ProfileStatus::Sealed,
        spec_document: "docs/source-profile-0.9.md",
        predecessor: Some("0.8"),
        supports_parsing: true,
        supports_elaboration: true,
        max_sequence_bound: 64,
        max_iteration_bound: 32,
        max_iteration_nesting: 1,
        max_iteration_work_product: 64,
        max_vector_lanes: 64,
        features: &["namespaces", "import_aliases"],
    },
    SourceProfileRecord {
        version: "0.10",
        status: ProfileStatus::Sealed,
        spec_document: "docs/source-profile-0.10.md",
        predecessor: Some("0.9"),
        supports_parsing: true,
        supports_elaboration: true,
        max_sequence_bound: 64,
        max_iteration_bound: 32,
        max_iteration_nesting: 1,
        max_iteration_work_product: 64,
        max_vector_lanes: 64,
        features: &["explicit_generics", "generic_specialization"],
    },
    SourceProfileRecord {
        version: "0.11",
        status: ProfileStatus::Sealed,
        spec_document: "docs/source-profile-0.11.md",
        predecessor: Some("0.10"),
        supports_parsing: true,
        supports_elaboration: true,
        max_sequence_bound: 64,
        max_iteration_bound: 32,
        max_iteration_nesting: 2,
        max_iteration_work_product: 4096,
        max_vector_lanes: 64,
        features: &["two_level_nested_iteration"],
    },
    SourceProfileRecord {
        version: "0.12",
        status: ProfileStatus::Sealed,
        spec_document: "docs/source-profile-0.12.md",
        predecessor: Some("0.11"),
        supports_parsing: true,
        supports_elaboration: true,
        max_sequence_bound: 64,
        max_iteration_bound: 32,
        max_iteration_nesting: 2,
        max_iteration_work_product: 4096,
        max_vector_lanes: 64,
        features: &[
            "binary64_arithmetic",
            "binary64_conversion",
            "trig_intrinsics",
        ],
    },
    SourceProfileRecord {
        version: "0.13",
        status: ProfileStatus::Sealed,
        spec_document: "docs/source-profile-0.13.md",
        predecessor: Some("0.12"),
        supports_parsing: true,
        supports_elaboration: true,
        max_sequence_bound: 1024,
        max_iteration_bound: 1024,
        max_iteration_nesting: 2,
        max_iteration_work_product: 1048576,
        max_vector_lanes: 64,
        features: &[
            "bool_negation",
            "bool_equality",
            "scalar_int_match",
            "sequential_iteration_reuse",
            "contextual_next_field",
            "repeat_sequence_literals",
            "lexical_shadowing",
            "raised_sequence_ceiling_1024",
            "raised_iteration_ceiling_1024",
            "structural_recursion",
        ],
    },
    SourceProfileRecord {
        version: "0.14",
        status: ProfileStatus::Current,
        spec_document: "docs/source-profile-0.14.md",
        predecessor: Some("0.13"),
        supports_parsing: true,
        supports_elaboration: true,
        max_sequence_bound: 1024,
        max_iteration_bound: 1024,
        max_iteration_nesting: 2,
        max_iteration_work_product: 1048576,
        max_vector_lanes: 64,
        features: &[
            "bulk_span_copy",
            "checked_view_narrowing",
            "checked_index_discharge",
        ],
    },
];

/// Look up the registry record for an exact profile version string.
pub fn source_profile_record(version: &str) -> Option<&'static SourceProfileRecord> {
    SOURCE_PROFILE_REGISTRY
        .iter()
        .find(|record| record.version == version)
}

/// Admitted per-level `iterate ... up_to N` ceiling for `profile`.
/// Returns `None` when the profile is unknown or has no iteration.
pub fn max_iteration_bound_for(profile: &str) -> Option<u32> {
    let bound = source_profile_record(profile)?.max_iteration_bound;
    (bound > 0).then_some(bound)
}

/// Admitted static work product for nested iteration in `profile`.
pub fn max_iteration_work_product_for(profile: &str) -> Option<u64> {
    let record = source_profile_record(profile)?;
    (record.max_iteration_work_product > 0).then_some(record.max_iteration_work_product)
}

/// Admitted nesting depth for `iterate` in `profile` (0 means iteration
/// itself is absent; 1 means no nesting).
pub fn max_iteration_nesting_for(profile: &str) -> Option<u32> {
    source_profile_record(profile).map(|record| record.max_iteration_nesting)
}

/// Admitted sequence/view length ceiling for `profile`. Zero means
/// sequences do not exist in that profile.
pub fn max_sequence_bound_for(profile: &str) -> Option<u32> {
    source_profile_record(profile).map(|record| record.max_sequence_bound)
}

/// Admitted vector/mask lane ceiling for `profile`.
pub fn max_vector_lanes_for(profile: &str) -> Option<u32> {
    source_profile_record(profile).map(|record| record.max_vector_lanes)
}

/// Owned, serializable mirror of [`SourceProfileRecord`] for the
/// machine-readable JSON export (`spec/source-profile-registry.json`).
/// The `const` registry above stays authoritative; this snapshot is
/// derived from it and pinned by `registry_snapshot_matches_artifact`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceProfileSnapshot {
    pub version: String,
    pub status: ProfileStatus,
    pub spec_document: String,
    pub predecessor: Option<String>,
    pub supports_parsing: bool,
    pub supports_elaboration: bool,
    pub max_sequence_bound: u32,
    pub max_iteration_bound: u32,
    pub max_iteration_nesting: u32,
    pub max_iteration_work_product: u64,
    pub max_vector_lanes: u32,
    pub features: Vec<String>,
}

impl From<&SourceProfileRecord> for SourceProfileSnapshot {
    fn from(record: &SourceProfileRecord) -> Self {
        Self {
            version: record.version.to_owned(),
            status: record.status,
            spec_document: record.spec_document.to_owned(),
            predecessor: record.predecessor.map(str::to_owned),
            supports_parsing: record.supports_parsing,
            supports_elaboration: record.supports_elaboration,
            max_sequence_bound: record.max_sequence_bound,
            max_iteration_bound: record.max_iteration_bound,
            max_iteration_nesting: record.max_iteration_nesting,
            max_iteration_work_product: record.max_iteration_work_product,
            max_vector_lanes: record.max_vector_lanes,
            features: record
                .features
                .iter()
                .map(|feature| (*feature).to_owned())
                .collect(),
        }
    }
}

/// Whole-registry snapshot for export and CI comparison.
pub fn source_profile_registry_snapshot() -> Vec<SourceProfileSnapshot> {
    SOURCE_PROFILE_REGISTRY
        .iter()
        .map(SourceProfileSnapshot::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_chain_is_linear() {
        assert!(!SOURCE_PROFILE_REGISTRY.is_empty());
        let mut previous: Option<&str> = None;
        for record in SOURCE_PROFILE_REGISTRY {
            assert_eq!(record.predecessor, previous, "{}", record.version);
            assert!(record.supports_parsing);
            assert!(record.supports_elaboration);
            previous = Some(record.version);
        }
        assert_eq!(SOURCE_PROFILE_REGISTRY.last().unwrap().version, "0.14");
    }

    #[test]
    fn support_predicate_matches_registry() {
        for record in SOURCE_PROFILE_REGISTRY {
            assert!(source_profile_supported(record.version));
        }
        assert!(!source_profile_supported("1.0"));
        assert!(!source_profile_supported("9.9"));
        assert!(source_profile_supported("0.14"));
        assert!(!source_profile_supported(""));
    }

    #[test]
    fn historical_ceilings_are_preserved() {
        let ceiling = |version: &str| source_profile_record(version).expect("registry record");
        assert_eq!(ceiling("0.4").max_iteration_bound, 32);
        assert_eq!(ceiling("0.11").max_iteration_bound, 32);
        assert_eq!(ceiling("0.11").max_iteration_nesting, 2);
        assert_eq!(ceiling("0.7").max_sequence_bound, 64);
        assert_eq!(ceiling("0.12").max_sequence_bound, 64);
        assert_eq!(ceiling("0.13").max_sequence_bound, 1024);
        assert_eq!(ceiling("0.13").max_iteration_bound, 1024);
        assert_eq!(ceiling("0.13").max_iteration_work_product, 1048576);
        assert_eq!(ceiling("0.14").max_sequence_bound, 1024);
        assert_eq!(ceiling("0.14").max_iteration_bound, 1024);
        assert_eq!(ceiling("0.14").max_iteration_work_product, 1048576);
    }

    #[test]
    fn print_registry_snapshot_for_artifact_refresh() {
        let snapshot = source_profile_registry_snapshot();
        println!(
            "REGISTRY-SNAPSHOT-BEGIN\n{}\nREGISTRY-SNAPSHOT-END",
            serde_json::to_string_pretty(&snapshot).expect("serialize registry")
        );
    }
}
