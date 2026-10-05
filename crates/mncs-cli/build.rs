//! Embed local source/build-input evidence in the Stage-0 reference executable.
//! This is a reproducible observation from this build, not an external attestation.
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    process::Command,
};

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn resolve_executable(configured: &str) -> PathBuf {
    let path = PathBuf::from(configured);
    if path.components().count() > 1 || path.is_absolute() {
        return path.canonicalize().expect("canonical build tool path");
    }
    std::env::split_paths(&std::env::var_os("PATH").expect("build PATH"))
        .map(|directory| directory.join(&path))
        .find_map(|candidate| candidate.canonicalize().ok())
        .expect("resolve build tool through PATH")
}

fn executable_identity(name: &str) -> serde_json::Value {
    let configured = std::env::var(name).expect("selected build tool path");
    let path = resolve_executable(&configured);
    println!("cargo:rerun-if-env-changed={name}");
    println!("cargo:rerun-if-changed={}", path.display());
    serde_json::json!({"configured_path":configured,"resolved_path":path.display().to_string(),"sha256":digest(&std::fs::read(path).expect("read build tool"))})
}

fn collect(path: &Path, files: &mut BTreeSet<PathBuf>) {
    if path.is_dir() {
        if matches!(
            path.file_name().and_then(|value| value.to_str()),
            Some("target" | ".git" | ".worktrees" | ".mncs" | "tests" | "examples" | "benches")
        ) {
            return;
        }
        let mut children: Vec<_> = std::fs::read_dir(path)
            .expect("enumerate Stage-0 build inputs")
            .map(|entry| entry.expect("read Stage-0 build input").path())
            .collect();
        children.sort();
        for child in children {
            collect(&child, files);
        }
    } else if path.is_file() {
        files.insert(path.canonicalize().expect("canonicalize Stage-0 input"));
    }
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    let crate_root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("Stage-0 workspace root")
        .canonicalize()
        .expect("canonical workspace");
    let mut inputs = BTreeSet::new();
    for input in [
        root.join("Cargo.toml"),
        root.join("Cargo.lock"),
        root.join("crates"),
    ] {
        collect(&input, &mut inputs);
    }
    let source_inputs: BTreeMap<String, String> = inputs
        .iter()
        .map(|path| {
            println!("cargo:rerun-if-changed={}", path.display());
            (
                path.strip_prefix(&root)
                    .expect("input in workspace")
                    .to_string_lossy()
                    .replace('\\', "/"),
                digest(&std::fs::read(path).expect("read source input")),
            )
        })
        .collect();
    for key in [
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "CARGO_TARGET_DIR",
        "CARGO_HOME",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    // Cargo must refresh the observation after a commit/worktree HEAD change.
    // Tracking HEAD alone misses same-branch commits, which move the branch
    // ref without rewriting HEAD; track the symbolic ref's path as well.
    if let Some(head) = git(&root, &["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={}", root.join(&head).display());
        if let Some(reference) = git(&root, &["symbolic-ref", "-q", "HEAD"]) {
            let path = git(&root, &["rev-parse", "--git-path", reference.as_str()]).unwrap_or(reference);
            if root.join(&path) != root.join(&head) {
                println!("cargo:rerun-if-changed={}", root.join(&path).display());
            }
        }
    }
    let revision = git(&root, &["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".to_owned());
    let status = git(
        &root,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )
    .unwrap_or_default();
    let changed: BTreeSet<String> = status
        .lines()
        .filter_map(|line| {
            (line.len() >= 4).then(|| {
                line[3..]
                    .rsplit(" -> ")
                    .next()
                    .unwrap_or(&line[3..])
                    .to_owned()
            })
        })
        .collect();
    let dirty_inputs: BTreeMap<String, String> = source_inputs
        .iter()
        .filter(|(path, _)| changed.contains(*path))
        .map(|(path, identity)| (path.clone(), identity.clone()))
        .collect();
    let rustc_identity = executable_identity("RUSTC");
    let rustc_path = rustc_identity["configured_path"].as_str().unwrap();
    let rustc = Command::new(&rustc_path)
        .arg("-vV")
        .output()
        .expect("query rustc");
    let cargo_identity = executable_identity("CARGO");
    let cargo_path = cargo_identity["configured_path"].as_str().unwrap();
    let cargo_version = Command::new(&cargo_path)
        .arg("--version")
        .output()
        .ok()
        .map(|value| String::from_utf8_lossy(&value.stdout).trim().to_owned());
    let features: BTreeMap<String, String> = std::env::vars()
        .filter(|(name, _)| name.starts_with("CARGO_FEATURE_"))
        .collect();
    let receipt = serde_json::json!({
        "schema_version": "mncs.language-build-receipt/1",
        "repository": "mncs-language",
        "producer_kind": "stage0-reference",
        "checkout": root.display().to_string(),
        "source_revision": revision,
        "source_inputs": source_inputs,
        "source_inputs_identity": digest(&serde_json::to_vec(&source_inputs).expect("serialize source inputs")),
        "dirty_content_identity": digest(&serde_json::to_vec(&dirty_inputs).expect("serialize dirty source inputs")),
        "dirty_input_count": dirty_inputs.len(),
        "build_configuration": {
            "profile": std::env::var("PROFILE").ok(), "target": std::env::var("TARGET").ok(),
            "opt_level": std::env::var("OPT_LEVEL").ok(), "debug": std::env::var("DEBUG").ok(),
            "rustflags": std::env::var("RUSTFLAGS").ok(), "encoded_rustflags": std::env::var("CARGO_ENCODED_RUSTFLAGS").ok(),
            "features": features, "cargo_path": cargo_path, "cargo_version": cargo_version,
            "toolchain_executables": {"cargo":cargo_identity,"rustc":rustc_identity},
            "rustc_path": rustc_path, "rustc_version": String::from_utf8_lossy(&rustc.stdout).trim(),
            "cargo_home": std::env::var("CARGO_HOME").ok()
        },
        "assurance": "local build observation; not independent attestation"
    });
    let bytes = serde_json::to_vec(&receipt).expect("serialize build receipt");
    let embedded =
        serde_json::json!({"identity": format!("sha256:{}", digest(&bytes)), "receipt": receipt});
    std::fs::write(
        PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"))
            .join("mncs-language-build-receipt.json"),
        serde_json::to_vec(&embedded).expect("serialize embedded receipt"),
    )
    .expect("write build receipt");
}
