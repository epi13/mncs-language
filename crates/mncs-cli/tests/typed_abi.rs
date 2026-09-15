//! The host-call interface identity must change with every semantic
//! signature change that could otherwise be hidden behind a stable function
//! name.  This is the compatibility fence for generated/validated bindings.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::Value;

static REQUEST_ID: AtomicUsize = AtomicUsize::new(0);

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn abi_for(source: &str) -> Value {
    let root = std::env::temp_dir().join(format!(
        "mncs-typed-abi-{}-{}",
        std::process::id(),
        REQUEST_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).expect("create ABI source directory");
    let path = root.join("interface.mncs");
    std::fs::write(&path, source).expect("write ABI source");
    let output = binary()
        .args(["abi", path.to_str().expect("ABI source path")])
        .output()
        .expect("run ABI inspection");
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "ABI inspection failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("ABI JSON")
}

const BASE: &str = "mncs 0.17;\nmodule abi.evolution;\nenum Mode { fast, safe }\nrecord Input { enabled: bool, mode: Mode, count: i32 }\nfn choose(input: Input, fallback: i32) -> (result: i32) { return input.count + fallback; }\n";
const REORDERED: &str = "mncs 0.17;\nmodule abi.evolution;\nenum Mode { fast, safe }\nrecord Input { enabled: bool, mode: Mode, count: i32 }\nfn choose(fallback: i32, input: Input) -> (result: i32) { return input.count + fallback; }\n";
const CHANGED_FIELD: &str = "mncs 0.17;\nmodule abi.evolution;\nenum Mode { fast, safe }\nrecord Input { enabled: bool, mode: Mode, count: i32, tag: i32 }\nfn choose(input: Input, fallback: i32) -> (result: i32) { return input.count + fallback; }\n";
const ADDED_VARIANT: &str = "mncs 0.17;\nmodule abi.evolution;\nenum Mode { fast, safe, guarded }\nrecord Input { enabled: bool, mode: Mode, count: i32 }\nfn choose(input: Input, fallback: i32) -> (result: i32) { return input.count + fallback; }\n";
const CHANGED_RETURN: &str = "mncs 0.17;\nmodule abi.evolution;\nenum Mode { fast, safe }\nrecord Input { enabled: bool, mode: Mode, count: i32 }\nfn choose(input: Input, fallback: i32) -> (result: bool) { return input.enabled; }\n";

#[test]
fn interface_identity_tracks_typed_abi_evolution() {
    let base = abi_for(BASE);
    let base_identity = base["interface_identity"].as_str().expect("base identity");
    assert_eq!(base["typed_call_schema_version"], "mncs.typed-call/1");
    assert_eq!(
        base["functions"]["choose"]["input_names"],
        serde_json::json!(["input", "fallback"])
    );
    let identity = |source: &str| {
        abi_for(source)["interface_identity"]
            .as_str()
            .expect("evolved identity")
            .to_owned()
    };
    assert_ne!(base_identity, identity(REORDERED));
    assert_ne!(base_identity, identity(CHANGED_FIELD));
    assert_ne!(base_identity, identity(ADDED_VARIANT));
    assert_ne!(base_identity, identity(CHANGED_RETURN));
}
