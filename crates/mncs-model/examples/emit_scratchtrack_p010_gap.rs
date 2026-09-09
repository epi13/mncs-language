//! Emit the ScratchTrack P-010 view-borrow capability-gap fixture.
//!
//! Run: `cargo run -p mncs-model --example emit_scratchtrack_p010_gap`
//! The printed JSON is checked in at
//! `examples/capability-gaps/scratchtrack-p010-view-borrow.json`.
//!
//! The gap records the exact-sequence/view-subtyping pressure resolved by
//! the Stage-A composability tranche: one reusable little-endian reader
//! over `[byte; up_to 64]` now serves every exact window that fits, via an
//! automatic zero-copy borrow. Status is PASS with the proof bound in.

use mncs_model::{CapabilityGap, GapObstruction, GapStatus};

fn main() {
    let gap = CapabilityGap::new(
        "scratchtrack/p010-view-borrow",
        "scratchtrack-pressure",
        "examples.subtype.windows",
        "examples/source/subtype-windows.mncs",
        "pass an exact [E; N] sequence wherever [E; up_to M] is expected with N <= M, with no copy, preserved bound, and preserved element identity",
        GapObstruction::Typing,
        "0.7",
        vec![
            "mncs-c11".to_owned(),
            "mncs-cranelift".to_owned(),
            "mncs-llvm-ir".to_owned(),
            "mncs-portable-wasm-mvp".to_owned(),
            "mncs-research-bytecode".to_owned(),
        ],
        "subtype_attempt.mncs (MNE133) superseded by the automatic borrow plus shared stdlib LE readers",
        GapStatus::Pass,
    )
    .expect("fixture gap")
    .with_optionals(
        None,
        None,
        Some("one hand-duplicated reader per exact width".to_owned()),
        vec!["exact borrows into a compatible bounded view with no copy".to_owned()],
        true,
        vec![
            "docs/source-profile-0.7.md: Exact-to-bounded-view borrow rule".to_owned(),
            "mncs.std.encoding.v1 read_u16_le/read_u32_le over [byte; up_to 64]".to_owned(),
            "examples/source/subtype-windows.mncs + examples/execution/subtype-windows-corpus.json via scripts/gen_subtype_windows_corpus.py (10 cases on all five backends)".to_owned(),
            "crates/mncs-cli/tests/abi_boundary.rs: exact_sequences_borrow_into_bounded_views_per_backend".to_owned(),
            "mncs-compiler borrow_tests: accept, oversize/element refusal, nested and tail-return borrows".to_owned(),
        ],
        vec![
            "view-to-view capacity relaxation is not part of the rule (declared caps are invariant)".to_owned(),
            "integer-overflow obligations in the readers remain UNKNOWN (P-003)".to_owned(),
        ],
    )
    .expect("fixture optionals");
    gap.verify_identity().expect("fixture identity");
    println!(
        "{}",
        serde_json::to_string_pretty(&gap).expect("fixture JSON")
    );
}
