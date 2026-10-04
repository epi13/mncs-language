//! INGEST-P-008: pinned standard-library distribution.
//!
//! Downstream in-process consumers must resolve a pinned `mncs.std.*`
//! identity without depending on the `mncs-language` working-tree path and
//! without copying the library into their own repository. The bundle is a
//! content-addressed document (per-module `sha256` plus a bundle identity
//! over the sorted pairs); loading re-verifies every hash and fails
//! closed. These tests pin the format, the checked-in artifact's
//! freshness, bundle-only transitive resolution (no filesystem), and the
//! refusal shapes (tamper, duplicates, unversioned names).

use mncs_compiler::bundle::{
    bundle_identity_for, collect_library_modules, pinned_bundle, BundledModule, StdlibBundle,
};
use mncs_compiler::{elaborate_program_with_resolver_and_modules, ModuleResolver};
use mncs_syntax::{parse, SourceArtifactKind, SourceEnvelope};

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

/// Stage F: standard-library sources now live in `mncs-stdlib/library/`.
fn stdlib_library_dir() -> String {
    let checkout = stdlib_checkout_dir();
    let dir = format!("{checkout}/library");
    assert!(
        std::path::Path::new(&dir).is_dir(),
        "mncs-stdlib checkout missing at {checkout}; set MNCS_STDLIB_ROOT"
    );
    dir
}

fn library_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(stdlib_library_dir())
}

fn parse_root(text: &str) -> mncs_syntax::AbstractSyntaxTree {
    let envelope = SourceEnvelope::inline(SourceArtifactKind::Program, "root", text.to_owned());
    let parsed = parse(&envelope);
    assert!(
        parsed.is_valid(),
        "fixture parses: {:?}",
        parsed.diagnostics
    );
    parsed.ast.expect("fixture parses")
}

#[test]
fn pinned_bundle_verifies_and_covers_the_library() {
    let bundle = pinned_bundle().expect("checked-in pin verifies");
    assert!(
        bundle.len() >= 50,
        "pin covers the library tree, got {} modules",
        bundle.len()
    );
    for name in [
        "mncs.std.chunk.v1",
        "mncs.std.encoding.v1",
        "mncs.std.text_utf8.v1",
        "mncs.std.text_map.v1",
        "mncs.std.json_cursor.v1",
        "mncs.core.sequences.v1",
    ] {
        assert!(
            bundle.module_text(name).is_some(),
            "pin serves {name}; modules: {:?}",
            bundle.module_names()
        );
    }
    assert!(
        bundle.bundle_identity().starts_with("mncs:stdlib-bundle:"),
        "identity names content: {}",
        bundle.bundle_identity()
    );
}

#[test]
fn checked_in_bundle_matches_the_library_tree() {
    // Stage F drift golden: the vendored pin (stdlib-pin/) must match a
    // fresh rebuild from the mncs-stdlib checkout tree. A stdlib edit
    // without re-pinning, or a canonical pin that was never re-vendored
    // here, fails here instead of drifting downstream.
    let collected = collect_library_modules(&library_dir()).expect("library tree collects");
    let rebuilt = StdlibBundle::assemble("mncs bundle generate", "test", collected)
        .expect("rebuild assembles");
    let pinned = pinned_bundle().expect("checked-in pin verifies");
    assert_eq!(
        rebuilt.bundle_identity(),
        pinned.bundle_identity(),
        "vendored pin drifts from the mncs-stdlib tree; re-vendor with `cp ../mncs-stdlib/dist/stdlib-bundle.json stdlib-pin/stdlib-bundle.json` (after ./tools/regen.sh over there)"
    );
    // And the vendored bytes must equal the canonical pin exactly, so a
    // hand-edited or partially copied lockfile cannot pass on identity
    // arithmetic alone.
    let canonical =
        std::fs::read_to_string(format!("{}/dist/stdlib-bundle.json", stdlib_checkout_dir()))
            .expect("canonical pin readable");
    let canonical_bundle = StdlibBundle::from_json(&canonical).expect("canonical pin verifies");
    assert_eq!(
        canonical_bundle.bundle_identity(),
        pinned.bundle_identity(),
        "vendored pin is not the canonical mncs-stdlib pin; re-vendor it"
    );
    assert_eq!(
        rebuilt.module_names(),
        pinned.module_names(),
        "module set drift"
    );
}

#[test]
fn bundle_resolver_serves_the_transitive_closure_without_filesystem() {
    // No filesystem anywhere: the resolver serves text_utf8, whose own
    // `use mncs.std.json_cursor.v1` resolves through the same bundle.
    let bundle = pinned_bundle().expect("checked-in pin verifies");
    let resolver = bundle.resolver();
    let ast = parse_root(
        "mncs 0.14;\nmodule app.bundleproof;\nuse mncs.std.text_utf8.v1;\nfn check(buf: [byte; 16], n: u64) -> (result: i64) {\n    let view: [byte; up_to 16] = buf[0..n];\n    return validate_generic<16>(view, n);\n}\n",
    );
    let (outcome, _, resolutions) = elaborate_program_with_resolver_and_modules(&ast, &resolver);
    assert!(
        outcome.is_ok(),
        "bundle-only elaboration succeeds: {:?}",
        outcome.err()
    );
    let mut declared: Vec<&str> = resolutions
        .iter()
        .map(|resolution| resolution.declared_module.as_str())
        .collect();
    declared.sort();
    declared.dedup();
    assert!(
        declared.contains(&"mncs.std.text_utf8.v1"),
        "direct import resolves, got {declared:?}"
    );
    assert!(
        declared.contains(&"mncs.std.json_cursor.v1"),
        "transitive import resolves through the bundle, got {declared:?}"
    );
}

#[test]
fn bundle_envelopes_name_their_pin() {
    // The P-007 import-failure chain identifies which pin served a
    // failing module, so bundle envelopes carry a Generated origin.
    let bundle = pinned_bundle().expect("checked-in pin verifies");
    let envelope = bundle
        .resolver()
        .resolve("mncs.std.chunk.v1")
        .expect("chunk resolves");
    let locator = envelope.origin.locator.clone().expect("bundle locator");
    assert!(
        locator.starts_with("stdlib-bundle:"),
        "locator names the bundle: {locator}"
    );
    assert!(
        locator.contains("mncs.std.chunk.v1"),
        "locator names the module: {locator}"
    );
    assert!(
        envelope.identity_is_valid(),
        "bundle envelope seals a valid identity"
    );
}

#[test]
fn unversioned_names_miss_the_bundle() {
    // The pin is exact and versioned: `mncs.std.chunk` (no `.v1`) never
    // matches, so development requests keep flowing to the filesystem.
    let bundle = pinned_bundle().expect("checked-in pin verifies");
    let resolver = bundle.resolver();
    assert!(resolver.resolve("mncs.std.chunk").is_none());
    assert!(resolver.resolve("mncs.std").is_none());
    assert!(resolver.resolve("app.local").is_none());
    assert!(resolver.resolve("mncs.std.chunk.v1").is_some());
}

#[test]
fn tampered_text_fails_closed() {
    let mut manifest: serde_json::Value =
        serde_json::from_str(mncs_compiler::bundle::PINNED_STDLIB_BUNDLE_JSON).expect("pin parses");
    let modules = manifest["modules"].as_array_mut().expect("modules array");
    let entry = modules
        .iter_mut()
        .find(|module| module["name"] == "mncs.std.chunk.v1")
        .expect("chunk entry");
    let text = entry["text"].as_str().expect("text").to_owned();
    entry["text"] = serde_json::Value::String(text.replacen("find_newline", "find_newlinX", 1));
    let tampered = serde_json::to_string(&manifest).expect("reserialize");
    let error = StdlibBundle::from_json(&tampered).unwrap_err();
    assert!(
        error.to_string().contains("mncs.std.chunk.v1"),
        "tamper names the module: {error}"
    );
}

#[test]
fn tampered_identity_fails_closed() {
    // Right text, wrong pin: the identity check catches re-pinning fraud
    // even when every per-module hash verifies.
    let mut manifest: serde_json::Value =
        serde_json::from_str(mncs_compiler::bundle::PINNED_STDLIB_BUNDLE_JSON).expect("pin parses");
    manifest["bundle_identity"] = serde_json::Value::String("mncs:stdlib-bundle:0".to_owned());
    let tampered = serde_json::to_string(&manifest).expect("reserialize");
    assert!(StdlibBundle::from_json(&tampered).is_err());
}

#[test]
fn duplicate_module_names_are_rejected() {
    let text = "mncs 0.10;\nmodule mncs.std.dup.v1;\nfn f() -> (result: u64) { return 0; }\n";
    let modules = vec![
        (
            "mncs.std.dup.v1".to_owned(),
            "std/dup.mncs".to_owned(),
            "0.10".to_owned(),
            text.to_owned(),
        ),
        (
            "mncs.std.dup.v1".to_owned(),
            "std/dup2.mncs".to_owned(),
            "0.10".to_owned(),
            text.to_owned(),
        ),
    ];
    let error = StdlibBundle::assemble("test", "test", modules).unwrap_err();
    assert!(
        error.to_string().contains("mncs.std.dup.v1"),
        "duplicate names the module: {error}"
    );
}

#[test]
fn bundle_identity_is_content_addressed() {
    // Identity depends on names and bytes only: order, paths, generator,
    // and commit do not move the pin.
    let left = [
        (
            "mncs.std.b.v1".to_owned(),
            "std/b.mncs".to_owned(),
            "0.10".to_owned(),
            "mncs 0.10;\nmodule mncs.std.b.v1;\n".to_owned(),
        ),
        (
            "mncs.std.a.v1".to_owned(),
            "std/a.mncs".to_owned(),
            "0.10".to_owned(),
            "mncs 0.10;\nmodule mncs.std.a.v1;\n".to_owned(),
        ),
    ];
    let modules: Vec<BundledModule> = left
        .iter()
        .map(|(name, path, profile, text)| {
            let digest = {
                use sha2::{Digest, Sha256};
                format!("{:x}", Sha256::digest(text.as_bytes()))
            };
            BundledModule {
                name: name.clone(),
                path: path.clone(),
                profile: profile.clone(),
                content_sha256: digest,
                text: text.clone(),
            }
        })
        .collect();
    let identity = bundle_identity_for(&modules);
    let mut reordered = modules.clone();
    reordered.reverse();
    assert_eq!(bundle_identity_for(&reordered), identity);
    let mut renamed_path = modules;
    renamed_path[0].path = "elsewhere.mncs".to_owned();
    assert_eq!(bundle_identity_for(&renamed_path), identity);
}
