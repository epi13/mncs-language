//! Profile-registry invariants (RFC 0036): implementation and profile
//! authority must not drift.
//!
//! - every registry record names an existing normative document;
//! - the checked-in JSON snapshot matches the authoritative registry;
//! - every version the parser accepts has exactly one registry record;
//! - the registry chain is linear from 0.1 to the current profile.

use mncs_syntax::{source_profile_registry_snapshot, SOURCE_PROFILE_REGISTRY};

fn workspace(name: &str) -> String {
    format!("{}/../../{name}", env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn every_record_names_an_existing_normative_document() {
    for record in SOURCE_PROFILE_REGISTRY {
        let path = workspace(record.spec_document);
        assert!(
            std::path::Path::new(&path).exists(),
            "profile {}: normative document missing: {}",
            record.version,
            record.spec_document
        );
    }
}

#[test]
fn registry_snapshot_matches_artifact() {
    let text = std::fs::read_to_string(workspace("spec/source-profile-registry.json"))
        .expect("read registry snapshot artifact");
    let artifact: serde_json::Value =
        serde_json::from_str(&text).expect("parse registry snapshot artifact");
    let live =
        serde_json::to_value(source_profile_registry_snapshot()).expect("serialize registry");
    assert_eq!(
        artifact, live,
        "spec/source-profile-registry.json drifted from the registry; \
         refresh it from `cargo test -p mncs-syntax --lib print_registry_snapshot -- --nocapture`"
    );
}

#[test]
fn every_accepted_version_has_exactly_one_record() {
    for version in [
        "0.1", "0.2", "0.3", "0.4", "0.5", "0.6", "0.7", "0.8", "0.9", "0.10", "0.11", "0.12",
        "0.13", "0.14",
    ] {
        assert!(
            mncs_syntax::source_profile_supported(version),
            "{version}: accepted version without registry support"
        );
        let matches = SOURCE_PROFILE_REGISTRY
            .iter()
            .filter(|record| record.version == version)
            .count();
        assert_eq!(matches, 1, "{version}: expected exactly one record");
    }
    assert!(
        !mncs_syntax::source_profile_supported("1.0"),
        "1.0 has no specification and must stay unsupported"
    );
}

#[test]
fn registry_chain_is_linear_to_current() {
    let mut previous: Option<&str> = None;
    for record in SOURCE_PROFILE_REGISTRY {
        assert_eq!(record.predecessor, previous);
        previous = Some(record.version);
    }
    let last = SOURCE_PROFILE_REGISTRY
        .last()
        .expect("registry is non-empty");
    assert_eq!(last.version, "0.14");
    assert_eq!(
        last.status,
        mncs_syntax::ProfileStatus::Current,
        "the chain tip must be the current profile"
    );
}
