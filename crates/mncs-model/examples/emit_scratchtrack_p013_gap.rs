//! Emit the ScratchTrack P-013 view-return capability-gap fixture.
//!
//! Run: `cargo run -p mncs-model --example emit_scratchtrack_p013_gap`
//! The printed JSON is checked in at
//! `examples/capability-gaps/scratchtrack-p013-view-returns.json`.
//!
//! The gap records the portable-WASM view-return divergence resolved by the
//! Stage-A sequence-ABI tranche: views over locally-built exact sequences
//! returned the first byte exactly and zeroed the rest (`[48,55]` observed
//! as `[48,0]`), and odd-offset packed slices were misread as cell-backed.
//! Status is PASS with the resolution evidence bound in; the original FAIL
//! phase is preserved in the ScratchTrack pressure ledger (P-013) and the
//! Stage-A development-evidence record, never overwritten here.

use mncs_model::{CapabilityGap, GapObstruction, GapStatus};

fn main() {
    let gap = CapabilityGap::new(
        "scratchtrack/p013-view-returns",
        "scratchtrack-pressure",
        "scratchtrack.probes.textprod_slice_attempt",
        "scratchtrack/mncs/probes/textprod_slice_attempt.mncs",
        "return a bounded byte view derived from a locally-built exact sequence with exact logical bytes on every executable backend",
        GapObstruction::Backend,
        "0.7",
        vec![
            "mncs-c11".to_owned(),
            "mncs-cranelift".to_owned(),
            "mncs-llvm-ir".to_owned(),
            "mncs-portable-wasm-mvp".to_owned(),
            "mncs-research-bytecode".to_owned(),
        ],
        "slice02/slice13 over buf[0..2]/buf[1..3] (textprod_slice_attempt.mncs + textprod_slice-corpus.json)",
        GapStatus::Pass,
    )
    .expect("fixture gap")
    .with_optionals(
        None,
        None,
        Some("fixed-width exact sequence returns; host-side byte production".to_owned()),
        vec!["cross-backend logical-value agreement on returned views".to_owned()],
        true,
        vec![
            "examples/source/abi-view-returns.mncs (18 value cases incl. P-013 vectors, empty/len-1/max-64/odd-even/param-slice/non-byte/repeated)".to_owned(),
            "examples/execution/abi-view-returns-corpus.json via scripts/gen_abi_view_returns_corpus.py".to_owned(),
            "crates/mncs-cli/tests/abi_boundary.rs: view_returns_agree_per_backend on all five executable backends".to_owned(),
            "examples/execution/abi-view-returns-traps-corpus.json: OOB-index and reversed-slice runtime_failure uniformly".to_owned(),
        ],
        vec![
            "integer-overflow obligations remain UNKNOWN (P-003; values verified, universal discharge missing)".to_owned(),
            "external-host return descriptors may still carry the lowering-internal bit-63 cell marker: versioned host ABI contract (P-011) must normalize-or-document".to_owned(),
        ],
    )
    .expect("fixture optionals");
    gap.verify_identity().expect("fixture identity");
    println!(
        "{}",
        serde_json::to_string_pretty(&gap).expect("fixture JSON")
    );
}
