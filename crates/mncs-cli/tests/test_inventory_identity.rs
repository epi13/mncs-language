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

fn run_declaration_inventory(binary: &PathBuf, source: &PathBuf) -> serde_json::Value {
    let output = Command::new(binary)
        .arg("declaration-inventory")
        .arg(source)
        .output()
        .expect("declaration-inventory command should start");
    assert!(
        output.status.success(),
        "declaration-inventory failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("declaration-inventory should emit JSON")
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

#[test]
fn generic_declaration_inventory_is_the_callable_authority() {
    let binary = PathBuf::from(
        env::var_os("CARGO_BIN_EXE_mncs").expect("Cargo should provide the mncs binary path"),
    );
    let root = env::temp_dir().join(format!("mncs-declaration-inventory-{}", std::process::id()));
    let source = root.join("source.mncs");
    fs::create_dir_all(&root).expect("inventory directory should be created");
    fs::write(
        &source,
        "mncs 0.17;\nmodule test.declaration_inventory;\nfn helper() -> (result: i64) { return 1; }\ntest one() -> (result: i64) { return helper(); }\n",
    )
    .expect("inventory source should be written");

    let document = run_declaration_inventory(&binary, &source);
    assert_eq!(document["schema_version"], "mncs.declaration-inventory/1");
    assert!(document["valid"].as_bool().unwrap());
    let callables = document["inventory"]["callables"].as_array().unwrap();
    let helper = callables
        .iter()
        .find(|item| item["name"] == "helper")
        .unwrap();
    let test = callables.iter().find(|item| item["name"] == "one").unwrap();
    assert_eq!(helper["callable_kind"], "function");
    assert!(helper["test_case_identity"].is_null());
    assert_eq!(test["callable_kind"], "test");
    assert!(test["test_case_identity"]
        .as_str()
        .unwrap()
        .starts_with("mncs:"));
    assert!(callables
        .iter()
        .all(|item| !item["name"].as_str().unwrap().starts_with("elaborate_")));
    assert!(document["inventory"]["inventory_identity"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));

    fs::remove_dir_all(root).expect("temporary inventory sources should be removable");
}

#[test]
fn language_inventory_uses_explicit_public_intrinsics() {
    let binary = PathBuf::from(
        env::var_os("CARGO_BIN_EXE_mncs").expect("Cargo should provide the mncs binary path"),
    );
    let output = Command::new(binary)
        .arg("language-inventory")
        .output()
        .expect("language-inventory command should start");
    assert!(output.status.success());
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("language-inventory should emit JSON");
    assert_eq!(document["schema_version"], "mncs.language-inventory/1");
    assert!(document["intrinsics"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| !item["name"].as_str().unwrap().starts_with("elaborate_")));
    assert!(document["inventory_identity"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
}
