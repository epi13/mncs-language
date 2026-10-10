use mncs_compiler::ReferenceCompiler;
use mncs_syntax::{SourceArtifactKind, SourceEnvelope};

fn front_end(profile: &str, capacity: u32) -> mncs_compiler::SourceFrontEndResult {
    let source = format!(
        "mncs {profile};\nmodule example.sequence_bound;\n\
         fn capacity(value: [byte; up_to {capacity}]) -> (result: u64) {{ return value.len; }}\n"
    );
    ReferenceCompiler::default().front_end(SourceEnvelope::inline(
        SourceArtifactKind::Program,
        "profile-019-sequence-bound",
        source,
    ))
}

#[test]
fn profile_019_admits_4096_and_profile_018_keeps_its_1024_limit() {
    let admitted = front_end("0.19", 4096);
    assert!(
        admitted.is_valid(),
        "Profile 0.19 accepts a 4096-capacity byte view: {:#?}",
        admitted.diagnostics
    );

    let rejected = front_end("0.18", 1025);
    assert!(
        !rejected.is_valid(),
        "Profile 0.18 preserves its 1024-capacity limit"
    );
    assert!(
        rejected
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "MNE105"),
        "the old profile rejects the unsupported bound: {:#?}",
        rejected.diagnostics
    );
}
