use std::fs;
use std::path::PathBuf;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

#[test]
fn forge_assets_are_explicit_consumer_integration() {
    let root = repository_root();
    let config_path = root.join("integration/forge/mncs-forge.toml");
    let config = fs::read_to_string(&config_path).expect("read Forge integration config");

    assert!(config.contains("root = \"../..\""));
    assert!(config.contains("integration/forge/provider.py"));
    assert!(config.contains("integration/forge/candidate"));
    assert!(!root.join("mncs-forge.toml").exists());
    assert!(!root.join("tools/mncs_language_forge_provider.py").exists());
    assert!(!root.join("forge/selection-policy.json").exists());
}
