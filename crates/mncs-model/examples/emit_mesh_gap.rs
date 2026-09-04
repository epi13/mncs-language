//! Emit the canonical qualified-record-construction capability-gap fixture.
//!
//! Run: `cargo run -p mncs-model --example emit_mesh_gap`
//! The printed JSON is checked in at
//! `examples/capability-gaps/qualified-record-construction.json`.

use mncs_model::{CapabilityGap, GapObstruction, GapStatus};

fn main() {
    let gap = CapabilityGap::new(
        "commons.mesh/qualified-record-construction",
        "commons-mesh-pressure",
        "commons.mesh.lattice_check",
        "examples/mesh/lattice_check.mncs",
        "construct an imported record value through a Profile 0.9 alias route",
        GapObstruction::Typing,
        "0.10",
        vec![
            "mncs-research-bytecode".to_owned(),
            "portable-wasm".to_owned(),
        ],
        "status.StatusPair { left: left, right: right } via `use ... as status`",
        GapStatus::Fail,
    )
    .expect("fixture gap")
    .with_optionals(
        None,
        None,
        Some("scalar-argument lattice join via status.dominate/2".to_owned()),
        vec!["FAIL dominates UNKNOWN dominates PASS".to_owned()],
        true,
        vec!["parser/semantic acceptance of the reproducer".to_owned()],
        vec!["exact resource cost of cross-module iteration".to_owned()],
    )
    .expect("fixture optionals");
    gap.verify_identity().expect("fixture identity");
    println!(
        "{}",
        serde_json::to_string_pretty(&gap).expect("fixture JSON")
    );
}
