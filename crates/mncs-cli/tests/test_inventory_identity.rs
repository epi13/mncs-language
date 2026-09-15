use std::{
    env, fs,
    path::PathBuf,
    process::{Command, Output},
};

fn run_inventory(binary: &PathBuf, source: &PathBuf) -> serde_json::Value {
    let output: Output = Command::new(binary)
        .arg("test-inventory")
        .arg(source)
        .output()
        .expect("test-inventory command should start");
    assert!(
        output.status.success(),
        "test-inventory failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("test-inventory should emit JSON")
}

#[test]
fn semantic_inventory_identity_is_independent_of_checkout_path() {
    let binary = PathBuf::from(
        env::var_os("CARGO_BIN_EXE_mncs").expect("Cargo should provide the mncs binary path"),
    );
    let root = env::temp_dir().join(format!("mncs-inventory-identity-{}", std::process::id()));
    let first = root.join("checkout-a/source.mncs");
    let second = root.join("checkout-b/source.mncs");
    let source =
        "mncs 0.17;\nmodule test.inventory_identity;\ntest one() -> (result: i64) { return 1; }\n";

    fs::create_dir_all(first.parent().expect("first parent")).expect("first directory");
    fs::create_dir_all(second.parent().expect("second parent")).expect("second directory");
    fs::write(&first, source).expect("first source");
    fs::write(&second, source).expect("second source");

    let first_document = run_inventory(&binary, &first);
    let second_document = run_inventory(&binary, &second);
    assert_eq!(
        first_document["inventory"], second_document["inventory"],
        "semantic inventory must not bind to an absolute checkout path"
    );

    fs::remove_dir_all(root).expect("temporary inventory sources should be removable");
}
