//! Emit the Profile 0.18 resource-bounded, cancellable process-effect gap.
//!
//! Run: `cargo run -p mncs-model --example emit_process_resource_handle_gap`
//! The output is checked in at
//! `examples/capability-gaps/process-resource-handle.json`.

use mncs_model::{CapabilityGap, GapObstruction, GapStatus};

fn main() {
    let gap = CapabilityGap::new(
        "MNCS-LANG-64AD712CD2DE",
        "mncs-forge",
        "mncs-forge:src/mncs_forge/resources/pressure-reproducers/process-resource-handle.mncs",
        "mncs-forge:src/mncs_forge/resources/pressure-reproducers/process-resource-handle.mncs",
        "run an external process under an explicit aggregate memory, swap, and process-count envelope; return bounded raw resource observations; and expose an owned handle that can interrupt an in-flight run",
        GapObstruction::Runtime,
        "0.18",
        vec!["mncs-host-process-provider".to_owned()],
        "process_run(request, envelope) is rejected with MNP216 because process_run accepts exactly one typed ProcessRequest; the current synchronous result has no in-flight cancellation handle",
        GapStatus::Fail,
    )
    .expect("fixture gap")
    .with_optionals(
        Some("mncs-language ff5e98c; Profile 0.18".to_owned()),
        Some("ff5e98c".to_owned()),
        Some("mncs.std.process.v1::process_run(ProcessRequest), with explicit argv, bounded environment and capture, exit status, and a deadline".to_owned()),
        vec![
            "memory and process limits apply to the complete owned process tree".to_owned(),
            "resource observations preserve unavailable and cleanup-unknown states".to_owned(),
            "cancellation cannot be reported complete before owned work is interrupted and reaped".to_owned(),
        ],
        true,
        vec![
            "Profile 0.18 accepts a source-visible generic resource request".to_owned(),
            "provider returns bounded typed process and resource observations".to_owned(),
            "an in-flight external process can be cancelled through its owned effect handle".to_owned(),
        ],
        vec![
            "compatible process effect contract and compiler lowering".to_owned(),
            "provider/runtime/embed support for owned cancellation and cleanup observation".to_owned(),
            "Linux cgroup-v2 realization for aggregate memory, swap, and process limits".to_owned(),
        ],
    )
    .expect("fixture optionals");
    gap.verify_identity().expect("fixture identity");
    println!(
        "{}",
        serde_json::to_string_pretty(&gap).expect("fixture JSON")
    );
}
