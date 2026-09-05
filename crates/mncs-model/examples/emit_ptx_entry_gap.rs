//! Emit the PTX kernel-entry annotation capability-gap fixture.
//!
//! Run: `cargo run -p mncs-model --example emit_ptx_entry_gap`
//! The printed JSON is checked in at
//! `examples/capability-gaps/ptx-kernel-entry-annotation.json`.
//!
//! Context: `--entry` now selects launchable PTX kernels explicitly, but the
//! selection lives outside MNCS source. A source-level kernel annotation
//! (effect/capability vocabulary declaring an export launchable) is the
//! remaining language pressure; this artifact records it structurally.

use mncs_model::{CapabilityGap, GapObstruction, GapStatus};

fn main() {
    let gap = CapabilityGap::new(
        "codegen.ptx/kernel-entry-annotation",
        "backend-conformance-pressure",
        "mncs-ptx64 kernel entry selection",
        "crates/mncs-cli/src/main.rs",
        "declare an MNCS export launchable as a PTX kernel from MNCS source",
        GapObstruction::SemanticValidation,
        "research-0.1",
        vec!["mncs-ptx64".to_owned()],
        "--entry bounded_min selects .entry emission; no source annotation exists",
        GapStatus::Fail,
    )
    .expect("fixture gap")
    .with_optionals(
        None,
        None,
        Some("--entry NAME compiler flag with ptx_kernel calling convention (explicit, outside source)".to_owned()),
        vec!["explicit selection only: no export becomes launchable by default".to_owned()],
        true,
        vec!["source-profile syntax for kernel/export annotation".to_owned()],
        vec!["profile-owned kernel semantics; eBPF program-type metadata analog".to_owned()],
    )
    .expect("fixture optionals");
    gap.verify_identity().expect("fixture identity");
    println!(
        "{}",
        serde_json::to_string_pretty(&gap).expect("fixture JSON")
    );
}
