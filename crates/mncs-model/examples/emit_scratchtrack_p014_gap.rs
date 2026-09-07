//! Emit the ScratchTrack P-014 nominal-import capability-gap fixture.
//!
//! Run: `cargo run -p mncs-model --example emit_scratchtrack_p014_gap`
//! The printed JSON is checked in at
//! `examples/capability-gaps/scratchtrack-p014-nominal-imports.json`.
//!
//! The gap records the imported-nominal-type pressure resolved by the
//! Stage-A composability tranche: the original reproducer named a
//! vocabulary (`PlatformEvent`) that no longer exists in the exporting
//! module, so it is bound here to the living proof instead — imported
//! finite/record identities elaborate and execute end to end across
//! modules. Status is PASS; the stale-reproducer analysis is preserved in
//! the Stage-A development-evidence record.

use mncs_model::{CapabilityGap, GapObstruction, GapStatus};

fn main() {
    let gap = CapabilityGap::new(
        "scratchtrack/p014-nominal-imports",
        "scratchtrack-pressure",
        "test.nominal.consumer",
        "examples/source/test/nominal/consumer.mncs",
        "import nominal finite and record identities across modules: signatures, returns, projection, finite values, match, nested calls, backend ABI identity",
        GapObstruction::Typing,
        "0.6",
        vec![
            "mncs-c11".to_owned(),
            "mncs-cranelift".to_owned(),
            "mncs-llvm-ir".to_owned(),
            "mncs-portable-wasm-mvp".to_owned(),
            "mncs-research-bytecode".to_owned(),
        ],
        "events_import_attempt.mncs (STALE: names PlatformEvent, absent from current platform.v1) superseded by test.nominal.machine/consumer pair",
        GapStatus::Pass,
    )
    .expect("fixture gap")
    .with_optionals(
        None,
        None,
        Some("per-module local re-declaration of shared vocabularies".to_owned()),
        vec!["imported nominals keep their declaring identity end to end".to_owned()],
        true,
        vec![
            "examples/source/test/nominal/machine.mncs + consumer.mncs (Phase/Evt/Transport)".to_owned(),
            "examples/execution/import-nominal-corpus.json via scripts/gen_import_nominal_corpus.py (10 cases PASS on all five backends)".to_owned(),
            "crates/mncs-cli/tests/abi_boundary.rs: imported_nominal_types_agree_per_backend".to_owned(),
            "crates/mncs-compiler/tests/module_imports.rs: imported_nominal_types_resolve_in_signatures_projection_and_match, projection_from_a_non_record_value_is_rejected".to_owned(),
        ],
        vec![
            "dotted construction (Os.Linux) is canonical; double-colon value paths are not term syntax (INVALID, not a gap)".to_owned(),
            "dotted QUALIFIED type paths require Profile 0.9+; unqualified unique imports work from 0.6".to_owned(),
        ],
    )
    .expect("fixture optionals");
    gap.verify_identity().expect("fixture identity");
    println!(
        "{}",
        serde_json::to_string_pretty(&gap).expect("fixture JSON")
    );
}
