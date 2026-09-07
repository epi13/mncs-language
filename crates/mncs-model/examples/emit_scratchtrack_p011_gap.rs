//! Emit the ScratchTrack P-011 host-ABI capability-gap fixture.
//!
//! Run: `cargo run -p mncs-model --example emit_scratchtrack_p011_gap`
//! The printed JSON is checked in at
//! `examples/capability-gaps/scratchtrack-p011-host-abi.json`.
//!
//! The gap records the unpublished-host-ABI pressure resolved by the
//! Stage-A sequence-ABI tranche: every host re-derived marshalling from
//! compiler internals. Status is PASS with the versioned contract bound
//! in; the reverse-engineering phase is preserved in the ScratchTrack
//! pressure ledger (P-011) and the Stage-A development-evidence record.

use mncs_model::{CapabilityGap, GapObstruction, GapStatus};

fn main() {
    let gap = CapabilityGap::new(
        "scratchtrack/p011-host-abi",
        "scratchtrack-pressure",
        "scratchtrack.mncsWasm",
        "scratchtrack/src/mncsWasm.ts",
        "target an MNCS WASM artifact from any host using only a versioned contract plus the per-module mncs abi report, without reading compiler internals",
        GapObstruction::Backend,
        "0.7",
        vec![
            "mncs-portable-wasm-mvp".to_owned(),
        ],
        "stageView/record reads reverse-mapped from mncs-codegen lower.rs/wasm.rs (packed descriptors, 8-byte-slot cells, host-buffer/reset protocol)",
        GapStatus::Pass,
    )
    .expect("fixture gap")
    .with_optionals(
        None,
        None,
        Some("per-shape tribal knowledge in one checked-in loader".to_owned()),
        vec!["no host reads compiler internals to marshal a call".to_owned()],
        true,
        vec![
            "spec/host-abi.md HOST-ABI version 1 (envelope, scalars, exact, views, staging protocol, results, failures, vectors)".to_owned(),
            "mncs abi carries host_abi_version binding each report to the contract".to_owned(),
            "executable vectors: abi-view-returns corpora, unsigned/bool/nested/over-capacity corpora on all five backends".to_owned(),
            "crates/mncs-cli/tests/abi_boundary.rs: abi_report_carries_the_host_abi_version".to_owned(),
        ],
        vec![
            "returned byte views may carry the provisional bit-63 cell selector: contract documents both representations, packed-only normalization is future work".to_owned(),
            "cross-call memory-growth discipline is a host-test obligation; experiment runs reset per case".to_owned(),
        ],
    )
    .expect("fixture optionals");
    gap.verify_identity().expect("fixture identity");
    println!(
        "{}",
        serde_json::to_string_pretty(&gap).expect("fixture JSON")
    );
}
