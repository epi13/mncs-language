//! Source-profile compatibility (RFC 0036): published profiles are
//! immutable semantic objects, and post-0.12 extensions live in Profiles
//! 0.13 (consolidation) and 0.14 (buffer pipelines).
//!
//! Every case below pins one direction of the evolution relation:
//! - an older profile keeps its *historical* acceptance/rejection and
//!   diagnostics for capabilities introduced later;
//! - Profile 0.13 admits the consolidated pressure-driven extensions;
//! - Profile 0.14 admits the buffer-pipeline tranche (span copy, checked
//!   view narrowing, checked-index discharge);
//! - `mncs 1.0` (no published specification) fails closed.
//!
//! The expected codes for the older-profile rows were reproduced against
//! the pre-0.13 implementation (lexical MNL002, MNP084/MNP157/MNP064 parse
//! walls, MNE121/MNE136/MNE146/MNE110/MNE142/MNE220 elaboration refusals)
//! and re-verified after gating; see
//! `docs/development-evidence/language-deep-review-repair-2026-09.md`.

use std::process::Command;

use serde_json::Value;

fn library(name: &str) -> String {
    format!("{}/../../library/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// Diagnostics for a committed fixture file (imports resolve relative to
/// the fixture's own directory).
fn file_diagnostics(path: &str) -> Vec<Value> {
    let output = binary()
        .args(["source-study", path])
        .env("MNCS_LIBRARY_PATH", library(""))
        .output()
        .expect("run source-study");
    let result: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
    result["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn study_diagnostics(text: &str) -> Vec<Value> {
    let dir = std::env::temp_dir().join(format!(
        "mncs-profile-compat-{}",
        text.len() % 1024
            + text.bytes().fold(0usize, |hash, byte| {
                hash.wrapping_mul(31).wrapping_add(byte as usize)
            }) % 65536
    ));
    std::fs::create_dir_all(&dir).expect("create workspace");
    let path = dir.join("case.mncs");
    std::fs::write(&path, text).expect("write case");
    let output = binary()
        .args(["source-study", &path.to_string_lossy()])
        .env("MNCS_LIBRARY_PATH", library(""))
        .output()
        .expect("run source-study");
    let result: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
    result["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

fn error_codes(text: &str) -> Vec<String> {
    study_diagnostics(text)
        .iter()
        .filter(|d| d.get("severity") == Some(&Value::String("error".to_owned())))
        .filter_map(|d| d["code"].as_str().map(str::to_owned))
        .collect()
}

fn expect_error(name: &str, text: &str, code: &str) {
    let errors = error_codes(text);
    assert!(
        errors.iter().any(|candidate| candidate == code),
        "{name}: expected {code} in {errors:?}"
    );
}

fn expect_clean(name: &str, text: &str) {
    let errors = error_codes(text);
    assert!(errors.is_empty(), "{name}: unexpected errors {errors:?}");
}

/// Older profiles reject boolean negation exactly as before Profile 0.13:
/// `!` never lexed there (MNL002).
#[test]
fn older_profiles_reject_logical_negation_lexically() {
    expect_error(
        "bang-012",
        "mncs 0.12;\nmodule compat.bang;\nfn main() -> (result: bool) {\n    return !true;\n}\n",
        "MNL002",
    );
    expect_error(
        "bang-010",
        "mncs 0.10;\nmodule compat.bang;\nfn main() -> (result: bool) {\n    return !true;\n}\n",
        "MNL002",
    );
}

/// Older profiles reject boolean equality with the historical integer
/// comparison gate (MNE121), not the 0.13 `BooleanCompare` operation.
#[test]
fn older_profiles_reject_bool_equality() {
    expect_error(
        "booleq-012",
        "mncs 0.12;\nmodule compat.booleq;\nfn main() -> (result: bool) {\n    return true == false;\n}\n",
        "MNE121",
    );
}

/// Older profiles reject integer match arms at the historical parse wall
/// (MNP084): scalar patterns did not exist there.
#[test]
fn older_profiles_reject_scalar_match_arms() {
    expect_error(
        "scalar-012",
        "mncs 0.12;\nmodule compat.scalar;\nfn probe(x: u64) -> (result: u64) {\n    return match x { 0 => 1, _ => 0 };\n}\n",
        "MNP084",
    );
}

/// Older profiles reject an integer match subject with the historical
/// finite-type message (MNE136): integers are not matchable there.
#[test]
fn older_profiles_reject_integer_match_subjects() {
    expect_error(
        "intsubject-012",
        "mncs 0.12;\nmodule compat.intsubject;\nfn probe(x: u64) -> (result: u64) {\n    return match x { _ => 0 };\n}\n",
        "MNE136",
    );
}

/// Older profiles reject any iteration-name reuse — sequential or nested —
/// with the historical function-wide rule (MNE146).
#[test]
fn older_profiles_reject_sequential_iteration_reuse() {
    expect_error(
        "reuse-012",
        "mncs 0.12;\nmodule compat.reuse;\nfn probe() -> (result: u64) {\n    iterate i up_to 2 carrying x: u64 = 0 {\n        next x = x + 1;\n    }\n    iterate i up_to 2 carrying y: u64 = x {\n        next y = y + 1;\n    }\n    return y;\n}\n",
        "MNE146",
    );
}

/// Older profiles reject `next` as a record field name: it is not an
/// identifier there, so the declaration keeps its historical parse error.
#[test]
fn older_profiles_reject_contextual_next_fields() {
    expect_error(
        "nextfield-012",
        "mncs 0.12;\nmodule compat.nextfield;\nrecord Link { next: u64 }\nfn probe(v: Link) -> (result: u64) {\n    return v.next;\n}\n",
        "MNP127",
    );
}

/// Profile 0.4 keeps its 1..=32 iteration bound (MNE142 at 33).
#[test]
fn profile_04_keeps_32_bound() {
    expect_error(
        "bound04",
        "mncs 0.4;\nmodule compat.bound04;\nfn probe() -> (result: u64) {\n    iterate i up_to 33 carrying x: u64 = 0 {\n        next x = x + 1;\n    }\n    return x;\n}\n",
        "MNE142",
    );
}

/// Profiles through 0.12 keep the 1..=32 per-level bound.
#[test]
fn older_profiles_keep_32_per_level_bound() {
    expect_error(
        "bound012",
        "mncs 0.12;\nmodule compat.bound012;\nfn probe() -> (result: u64) {\n    iterate i up_to 64 carrying x: u64 = 0 {\n        next x = x + 1;\n    }\n    return x;\n}\n",
        "MNE142",
    );
}

/// Profiles through 0.12 keep the 64-element admitted sequence ceiling in
/// type spellings.
#[test]
fn older_profiles_keep_64_sequence_ceiling() {
    expect_error(
        "seq65-012",
        "mncs 0.12;\nmodule compat.seq65;\nfn probe(v: [i64; 65]) -> (result: i64) {\n    return v[0];\n}\n",
        "MNE105",
    );
}

/// Older profiles refuse u64 traversal domains (the historical MNB101
/// rule, enforced at elaboration as MNE194 so model validation stays
/// profile-agnostic).
#[test]
fn older_profiles_refuse_u64_traversal_domains() {
    expect_error(
        "u64dom-012",
        "mncs 0.12;\nmodule compat.u64dom;\nfn probe(a: [u64; 4]) -> (result: u64) {\n    iterate i over a carrying s: u64 = 0 {\n        next s = s + a[i];\n    }\n    return s;\n}\n",
        "MNE194",
    );
}

/// Older profiles refuse same-scope shadowing with the historical
/// ambiguity diagnostic (MNE110).
#[test]
fn older_profiles_refuse_same_scope_shadowing() {
    expect_error(
        "shadow-012",
        "mncs 0.12;\nmodule compat.shadow;\nfn probe() -> (result: i64) {\n    let x: i64 = 1;\n    let x: i64 = x + 1;\n    return x;\n}\n",
        "MNE110",
    );
}

/// Older profiles refuse unary-negative literal atoms at the historical
/// expression wall (MNP064).
#[test]
fn older_profiles_refuse_negative_literal_atoms() {
    expect_error(
        "neg-012",
        "mncs 0.12;\nmodule compat.neg;\nfn add2(a: i64, b: i64) -> (result: i64) {\n    return a + b;\n}\nfn probe() -> (result: i64) {\n    return add2(-5, -2);\n}\n",
        "MNP064",
    );
}

/// Older profiles refuse repeat literals at the historical sequence wall
/// (MNP157).
#[test]
fn older_profiles_refuse_repeat_literals() {
    expect_error(
        "repeat-012",
        "mncs 0.12;\nmodule compat.repeat;\nfn probe() -> (result: i64) {\n    let row: [i64; 3] = [7; 3];\n    return row[1];\n}\n",
        "MNP157",
    );
}

/// Older profiles refuse to infer generic arguments: explicit `<...>` was
/// required (historical MNE220 wording preserved verbatim).
#[test]
fn older_profiles_refuse_generic_inference() {
    let text = "mncs 0.12;\nmodule compat.infer;\nfn id<T>(value: T) -> (result: T) {\n    return value;\n}\nfn probe() -> (result: i64) {\n    return id(1);\n}\n";
    let diagnostics = study_diagnostics(text);
    let messages: Vec<String> = diagnostics
        .iter()
        .filter(|d| d["code"] == "MNE220")
        .filter_map(|d| d["message"].as_str().map(str::to_owned))
        .collect();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("inference is not available")),
        "inference-012: expected historical MNE220, got {messages:?}"
    );
}

/// `mncs 1.0` has no published specification and no registry record: it
/// fails closed with a precise diagnostic instead of inheriting 0.x
/// semantics through numeric comparison.
#[test]
fn profile_10_fails_closed_without_specification() {
    let text = "mncs 1.0;\nmodule compat.one;\nfn main() -> (result: u64) {\n    return 1;\n}\n";
    let diagnostics = study_diagnostics(text);
    let messages: Vec<String> = diagnostics
        .iter()
        .filter(|d| d["code"] == "MNP008")
        .filter_map(|d| d["message"].as_str().map(str::to_owned))
        .collect();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("no published specification")),
        "profile-10: expected MNP008 fail-closed, got {diagnostics:?}"
    );
}

/// Profile 0.13 admits the consolidated extensions: negation, boolean
/// equality, scalar match, sequential reuse, contextual `next` fields,
/// raised ceilings, repeat literals, shadowing, negative literals, u64
/// domains, and generic inference.
#[test]
fn profile_013_admits_consolidated_extensions() {
    expect_clean(
        "admit-013",
        "mncs 0.13;\nmodule compat.admit;\nrecord Link { next: u64, after: u64 }\nfn id<T>(value: T) -> (result: T) {\n    return value;\n}\nfn probe(a: [u64; 4], x: u64) -> (result: u64) {\n    let flag: bool = !true;\n    let same: bool = flag == false;\n    let picked: u64 = match x { 0 => 10, _ => 0 };\n    let link: Link = Link { next: 1, after: 2 };\n    let hops: u64 = link.next;\n    let row: [i64; 3] = [7; 3];\n    let shadow: i64 = 1;\n    let shadow: i64 = shadow + row[1];\n    let negated: i64 = id(-5);\n    let ok: bool = same && (negated == 0 - 5);\n    let gate: u64 = match ok { true => 1, false => 0 };\n    let inferred: u64 = id(picked);\n    iterate i over a carrying s: u64 = picked {\n        next s = s + a[i];\n    }\n    iterate i up_to 64 carrying t: u64 = s {\n        next t = t + 1;\n    }\n    return t + picked + gate + inferred + hops;\n}\n",
    );
}

/// Generic traversal definitions defer the admitted sequence ceiling to
/// instantiation: a `[T; N]` traversal elaborates under older profiles
/// against the absolute model ceiling, so profile-portable generic
/// libraries (core status/sequence/geometry) keep elaborating.
#[test]
fn generic_traversal_definition_admitted_below_013() {
    expect_clean(
        "gentraverse-012",
        "mncs 0.12;\nmodule compat.gentraverse;\nfn total<N: Nat>(xs: [i64; N]) -> (result: i64) {\n    iterate i over xs carrying s: i64 = 0 {\n        next s = s + xs[i];\n    }\n    return s;\n}\n",
    );
}

/// An explicit over-ceiling Nat argument still fails closed at its call
/// site (MNE225) under the caller's admitted ceiling.
#[test]
fn explicit_over_ceiling_nat_arg_refused() {
    expect_error(
        "genceil-012",
        "mncs 0.12;\nmodule compat.genceil;\nfn total<N: Nat>(xs: [i64; N]) -> (result: i64) {\n    iterate i over xs carrying s: i64 = 0 {\n        next s = s + xs[i];\n    }\n    return s;\n}\nfn probe() -> (result: i64) {\n    let row: [i64; 4] = [1, 2, 3, 4];\n    return total<100>(row);\n}\n",
        "MNE225",
    );
}

/// Profiles through 0.13 refuse the buffer-pipeline intrinsics at parse:
/// `copy_span` and `checked_index` never lexed there.
#[test]
fn older_profiles_refuse_buffer_pipeline_intrinsics() {
    expect_error(
        "copyspan-013",
        "mncs 0.13;\nmodule compat.copyspan;\nfn probe(dst: [byte; 8], src: [byte; 8]) -> (result: [byte; 8]) {\n    return copy_span(dst, 0, src, 0, 8);\n}\n",
        "MNP207",
    );
    expect_error(
        "checkedindex-013",
        "mncs 0.13;\nmodule compat.checkedindex;\nfn probe(buf: [byte; 8], i: u64) -> (result: byte) {\n    let c: u64 = checked_index(buf, i);\n    return buf[c];\n}\n",
        "MNP209",
    );
}

/// Profiles through 0.13 keep the historical view-compatibility rule: a
/// wider view does not satisfy a narrower expectation (MNE188); only
/// 0.14 admits the checked narrowing.
#[test]
fn older_profiles_refuse_view_narrowing() {
    expect_error(
        "narrow-013",
        "mncs 0.13;\nmodule compat.narrow;\nfn read(window: [byte; up_to 64]) -> (result: u64) {\n    return (window[0] as u64);\n}\nfn probe(buf: [byte; up_to 1024]) -> (result: u64) {\n    let window: [byte; up_to 64] = buf[0..64];\n    return read(window);\n}\n",
        "MNE188",
    );
}

/// Profile 0.14 admits the buffer-pipeline tranche end to end.
#[test]
fn profile_014_admits_buffer_pipelines() {
    expect_clean(
        "admit-014",
        "mncs 0.14;\nmodule compat.admit14;\nfn read(window: [byte; up_to 64], i: u64) -> (result: byte) {\n    let c: u64 = checked_index(window, i);\n    return window[c];\n}\nfn moved(dst: [byte; 8], src: [byte; 8]) -> (result: [byte; 8]) {\n    return copy_span(dst, 0, src, 0, 8);\n}\nfn probe(src: [byte; 1024], i: u64) -> (result: byte) {\n    let window: [byte; up_to 64] = src[0..64];\n    return read(window, i);\n}\n",
    );
}

/// A concrete substitution that exceeds the root program's admitted
/// ceiling fails closed after specialization (MNE182): the 0.13 caller
/// admits N=500 at its call site, but the 0.10 root refuses the
/// 500-wide traversal substituted into the generic template.
#[test]
fn over_ceiling_specialization_refused_under_narrow_root() {
    let diagnostics = file_diagnostics(&example("source/generic-ceilings/narrow_root.mncs"));
    let messages: Vec<String> = diagnostics
        .iter()
        .filter(|d| d["code"] == "MNE182")
        .filter_map(|d| d["message"].as_str().map(str::to_owned))
        .collect();
    assert!(
        messages
            .iter()
            .any(|message| message.contains("specialized traversal bound exceeds")),
        "narrow-root: expected specialization MNE182, got {diagnostics:?}"
    );
}
