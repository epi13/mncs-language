use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256};

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "mncs-native-application-identity-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create isolated application fixture");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn library_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../library")
}

#[test]
fn run_app_supplies_native_sha256_identity_at_the_application_boundary() {
    let temporary = TemporaryDirectory::new();
    let identity = "n".repeat(1024);
    let expected = Sha256::digest(identity.as_bytes());
    let source_path = temporary.path().join("identity.mncs");
    let descriptor_path = temporary.path().join("identity.json");
    let cache_path = temporary.path().join("cache");
    let source = r#"mncs 0.19;
module test.native_application_identity;

use mncs.std.application.v1 as app;

fn main(context: app.ApplicationContext) -> (result: app.ApplicationExit) {
    return app.success(context.application_identity);
}
"#;
    let descriptor = serde_json::json!({
        "schema_version": "mncs.native-application/1",
        "application_identity": identity,
        "source": source_path,
        "module": "test.native_application_identity",
        "entry_function": "main",
        "profile": "0.19",
        "libraries": [library_dir()],
        "environment_keys": [],
        "required_capabilities": []
    });
    std::fs::write(&source_path, source).expect("write native application source");
    std::fs::write(
        &descriptor_path,
        serde_json::to_vec(&descriptor).expect("serialize application descriptor"),
    )
    .expect("write native application descriptor");

    for attempt in 0..2 {
        let output = Command::new(env!("CARGO_BIN_EXE_mncs"))
            .args([
                "run-app",
                descriptor_path.to_str().expect("UTF-8 descriptor path"),
                "--cache-dir",
                cache_path.to_str().expect("UTF-8 cache path"),
                "--library",
                library_dir().to_str().expect("UTF-8 library path"),
            ])
            .env("MNCS_NATIVE_CACHE_TRACE", "1")
            .output()
            .expect("run native application");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, expected.as_slice());
        if attempt == 1 {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.matches("status=hit").count() >= 2,
                "application and SHA-256 artifacts should both hit the shared cache: {}",
                stderr
            );
        }
    }
}
