//! Emit the embeddable-execution capability-gap fixture.
//!
//! Run: `cargo run -p mncs-model --example emit_exec_gap`
//! The printed JSON is checked in at
//! `examples/capability-gaps/embeddable-execution.json`.

use mncs_model::{CapabilityGap, GapObstruction, GapStatus};

fn main() {
    let gap = CapabilityGap::new(
        "commons.mesh/embeddable-execution",
        "commons-mesh-pressure",
        "mncs experiment run/execute surface",
        "crates/mncs-cli/src/main.rs",
        "compile an MNCS semantic function once, then decide many bounded host batches against the compiled artifact",
        GapObstruction::Runtime,
        "0.10",
        vec![
            "mncs-research-bytecode".to_owned(),
            "mncs-portable-wasm-mvp".to_owned(),
        ],
        "per-sync-batch `experiment run` costs ~8s dominated by rebuild; `experiment execute` refuses substituted corpora by design; no .mncs to program.json emission exists",
        GapStatus::Fail,
    )
    .expect("fixture gap")
    .with_optionals(
        None,
        None,
        Some("per-batch `experiment run` with bounded corpora (correct, ~8s per batch)".to_owned()),
        vec!["frozen replication refuses substituted inputs".to_owned()],
        true,
        vec!["timed per-batch runs over the mesh named corpus on both backends".to_owned()],
        vec!["persistent execution service or program.json emission design".to_owned()],
    )
    .expect("fixture optionals");
    gap.verify_identity().expect("fixture identity");
    println!(
        "{}",
        serde_json::to_string_pretty(&gap).expect("fixture JSON")
    );
}
