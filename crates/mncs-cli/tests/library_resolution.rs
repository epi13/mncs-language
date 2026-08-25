use std::fs;
use std::process::Command;

use serde_json::Value;

fn library_dir() -> String {
    format!("{}/../../library", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

/// A consumer outside the library tree must resolve `mncs.core.status.v1`
/// when the library root is exported through `MNCS_LIBRARY_PATH`, and must
/// fail closed with a resolution diagnostic when it is not.
#[test]
fn external_consumer_resolves_core_through_library_path() {
    let workspace = std::env::temp_dir().join(format!(
        "mncs-library-path-{}",
        std::process::id()
    ));
    fs::create_dir_all(&workspace).expect("workspace directory");
    let source_path = workspace.join("consumer.mncs");
    fs::write(
        &source_path,
        r#"mncs 0.6;

module tmp.consumer;

use mncs.core.status.v1;

fn soften(left: Status, right: Status) -> (result: Status) {
    return dominate(left, right);
}
"#,
    )
    .expect("write consumer");

    // Without a library root the import cannot be satisfied: elaboration
    // fails closed with the unresolvable-import diagnostic.
    let bare = binary()
        .args(["validate"])
        .arg(&source_path)
        .output()
        .expect("validate without library path");
    let bare_text = String::from_utf8_lossy(&bare.stdout);
    assert!(
        !bare.status.success() && bare_text.contains("MNE173"),
        "expected MNE173 without MNCS_LIBRARY_PATH: {bare_text}"
    );

    // With the root exported, the same source elaborates and validates.
    let resolved = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(["validate"])
        .arg(&source_path)
        .output()
        .expect("validate with library path");
    assert!(
        resolved.status.success(),
        "expected success with MNCS_LIBRARY_PATH: {}",
        String::from_utf8_lossy(&resolved.stdout)
    );

    fs::remove_dir_all(&workspace).ok();
}
