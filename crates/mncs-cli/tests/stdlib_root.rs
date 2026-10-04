//! Stage F: standard-library root discovery precedence.
//!
//! The toolchain resolves `mncs.*` imports from the `mncs-stdlib`
//! repository: explicit `MNCS_STDLIB_ROOT` wins, else the `mncs-stdlib`
//! sibling of the language checkout backing the binary, else no default
//! root. Set-but-empty `MNCS_STDLIB_ROOT` disables the default for
//! hermetic invocations. Explicit `MNCS_LIBRARY_PATH` entries always
//! apply on top.

use std::fs;
use std::process::Command;

/// Stage F: the `mncs-stdlib` checkout backing these tests: explicit
/// `MNCS_STDLIB_ROOT` wins, else the `mncs-stdlib` sibling checkout.
/// Fails closed with a clear message when absent.
fn stdlib_checkout_dir() -> String {
    // Test inputs need a real checkout: an explicitly empty variable
    // (the CLI's hermetic spelling) falls through to the sibling here.
    let explicit = std::env::var("MNCS_STDLIB_ROOT")
        .ok()
        .filter(|root| !root.trim().is_empty());
    let checkout =
        explicit.unwrap_or_else(|| format!("{}/../../../mncs-stdlib", env!("CARGO_MANIFEST_DIR")));
    assert!(
        std::path::Path::new(&checkout).is_dir(),
        "mncs-stdlib checkout missing at {checkout}; set MNCS_STDLIB_ROOT"
    );
    checkout
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn consumer(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("mncs-stdlib-root-{tag}-{}", std::process::id()));
    fs::create_dir_all(&dir).expect("workspace directory");
    let source_path = dir.join("consumer.mncs");
    fs::write(
        &source_path,
        "mncs 0.6;\n\nmodule tmp.consumer;\n\nuse mncs.core.status.v1;\n\nfn soften(left: Status, right: Status) -> (result: Status) {\n    return dominate(left, right);\n}\n",
    )
    .expect("write consumer");
    source_path
}

/// Bare invocation (no library env at all) resolves through the
/// toolchain's sibling default in a family checkout.
#[test]
fn bare_invocation_resolves_via_sibling_default() {
    // Lone-checkout runs have no sibling to discover; the precondition
    // is absent, so there is nothing to assert (family CI always has it).
    let sibling = format!("{}/library", stdlib_checkout_dir());
    if std::env::var("MNCS_STDLIB_ROOT").is_err() && !std::path::Path::new(&sibling).is_dir() {
        eprintln!("note: no mncs-stdlib sibling; skipping default-discovery assertion");
        return;
    }
    let source_path = consumer("bare");
    let output = binary()
        .env_remove("MNCS_LIBRARY_PATH")
        .env_remove("MNCS_STDLIB_BUNDLE")
        .env_remove("MNCS_STDLIB_ROOT")
        .args(["validate"])
        .arg(&source_path)
        .output()
        .expect("validate with sibling default");
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        output.status.success(),
        "sibling default must resolve stdlib imports: {text}"
    );
    fs::remove_dir_all(source_path.parent().unwrap()).ok();
}

/// Set-but-empty `MNCS_STDLIB_ROOT` disables the default: the same
/// source fails closed with the unresolvable-import diagnostic.
#[test]
fn empty_stdlib_root_disables_the_default() {
    let source_path = consumer("empty");
    let output = binary()
        .env_remove("MNCS_LIBRARY_PATH")
        .env_remove("MNCS_STDLIB_BUNDLE")
        .env("MNCS_STDLIB_ROOT", "")
        .args(["validate"])
        .arg(&source_path)
        .output()
        .expect("validate with disabled default");
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        !output.status.success() && text.contains("MNE173"),
        "expected MNE173 with disabled default: {text}"
    );
    fs::remove_dir_all(source_path.parent().unwrap()).ok();
}

/// Explicit `MNCS_STDLIB_ROOT` redirects resolution to that checkout.
#[test]
fn explicit_stdlib_root_wins() {
    let source_path = consumer("explicit");
    let output = binary()
        .env_remove("MNCS_LIBRARY_PATH")
        .env_remove("MNCS_STDLIB_BUNDLE")
        .env("MNCS_STDLIB_ROOT", stdlib_checkout_dir())
        .args(["validate"])
        .arg(&source_path)
        .output()
        .expect("validate with explicit root");
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        output.status.success(),
        "explicit MNCS_STDLIB_ROOT must resolve: {text}"
    );
    fs::remove_dir_all(source_path.parent().unwrap()).ok();
}

/// An explicit root without a library is an honest miss, not a crash
/// and not a silent fallback to the sibling.
#[test]
fn missing_explicit_root_fails_closed_without_fallback() {
    let source_path = consumer("missing");
    let empty = std::env::temp_dir().join(format!("mncs-stdlib-empty-{}", std::process::id()));
    fs::create_dir_all(&empty).expect("empty dir");
    let output = binary()
        .env_remove("MNCS_LIBRARY_PATH")
        .env_remove("MNCS_STDLIB_BUNDLE")
        .env("MNCS_STDLIB_ROOT", &empty)
        .args(["validate"])
        .arg(&source_path)
        .output()
        .expect("validate with missing root");
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    assert!(
        !output.status.success() && text.contains("MNE173"),
        "expected MNE173 for missing explicit root: {text}"
    );
    fs::remove_dir_all(source_path.parent().unwrap()).ok();
    fs::remove_dir_all(&empty).ok();
}
