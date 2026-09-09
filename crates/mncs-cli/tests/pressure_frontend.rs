//! Frontend pressure regression: hygienic temps, u64 domains, negative
//! literals, nested-call composition. Each case pins the `mncs-math`
//! pressure item against current `main` so the language cannot regress
//! without an external reproducer.

use std::process::Command;

use serde_json::Value;

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn study(name: &str, source_text: &str) -> Value {
    let dir = std::env::temp_dir().join(format!("mncs-pressure-frontend-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create workspace");
    let path = dir.join(format!("{name}.mncs"));
    std::fs::write(&path, source_text).expect("write case");
    let output = binary()
        .args(["source-study", &path.to_string_lossy()])
        .output()
        .expect("run source-study");
    serde_json::from_slice(&output.stdout).expect("front-end JSON")
}

fn errors(result: &Value) -> Vec<String> {
    result["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter(|d| d["severity"] == "error")
        .filter_map(|d| d["code"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn hygienic_temps_never_alias_short_user_bindings() {
    // MNB011: record literal and ||-chain temps must not collide with c0/b3.
    let ok = study("mnb011", 
        "mncs 0.10;\nmodule probe.mnb011_v1;\nrecord DualZ { x: i64, dx: i64 }\nfn no_literal(c0: i64, c1: i64, t: DualZ) -> (result: DualZ) {\n    return DualZ { x: c1, dx: 0 };\n}\nfn bcast_ok4(a3: i64, b3: i64) -> (result: bool) {\n    let c3: bool = a3 == b3 || a3 == 1 || b3 == 1;\n    return c3;\n}\n",
    );
    let codes = errors(&ok);
    assert!(
        !codes.iter().any(|c| c == "MNB011"),
        "MNB011 regressed: {codes:?}"
    );
}

#[test]
fn u64_iterate_domains_resolve_like_i64_and_f64() {
    // MNB101: [u64; N] traverses exactly like [i64; N] and [f64; N].
    let ok = study("mnb101", 
        "mncs 0.13;\nmodule probe.udomain_v1;\nfn usum(a: [u64; 4]) -> (result: u64) {\n    iterate i over a carrying s: u64 = 0 {\n        next s = s + a[i];\n    }\n    return s;\n}\nfn entry_usum(x0: u64, x1: u64, x2: u64, x3: u64) -> (result: u64) {\n    return usum([x0, x1, x2, x3]);\n}\n",
    );
    let codes = errors(&ok);
    assert!(
        !codes.iter().any(|c| c == "MNB101"),
        "MNB101 regressed: {codes:?}"
    );
}

#[test]
fn negative_literals_are_valid_expression_arguments() {
    // MNP064: f(-5, -2), floats, nested calls, record fields, subtraction.
    let ok = study("mnp064", 
        "mncs 0.13;\nmodule probe.negarg_v1;\nrecord Pt { x: i64, y: i64 }\nfn add2(a: i64, b: i64) -> (result: i64) {\n    return a + b;\n}\nfn sub2(a: i64, b: i64) -> (result: i64) {\n    return a - b;\n}\nfn entry_neg() -> (result: i64) {\n    return add2(-5, -2);\n}\nfn entry_sub(x: i64, y: i64) -> (result: i64) {\n    return sub2(x, y);\n}\nfn entry_nested(x: i64) -> (result: i64) {\n    return add2(sub2(x, 1), -5);\n}\nfn entry_rec() -> (result: Pt) {\n    return Pt { x: -3, y: -4 };\n}\nfn entry_float(a: f64) -> (result: f64) {\n    return a + -2.5;\n}\n",
    );
    let codes = errors(&ok);
    assert!(
        !codes.iter().any(|c| c == "MNP064"),
        "MNP064 regressed: {codes:?}"
    );
    // Subtraction itself still parses (no ambiguity introduced).
    assert!(codes.is_empty(), "unexpected errors: {codes:?}");
}

#[test]
fn general_negation_stays_refused() {
    // Only literals negate; -x and --5 remain grammar errors (spell 0 - x).
    let bad = study("tneg", 
        "mncs 0.10;\nmodule probe.negref_v1;\nfn entry_negx(x: i64) -> (result: i64) {\n    return -x;\n}\n",
    );
    let codes = errors(&bad);
    assert!(
        codes.iter().any(|c| c == "MNP064"),
        "general -x should stay refused, got: {codes:?}"
    );
}

#[test]
fn nested_call_composition_elaborates() {
    // MNE133/135 negative result: sub(x, mul(a,b)) is composition, not a ban.
    let ok = study("nest", 
        "mncs 0.10;\nmodule probe.nestarg_v1;\nfn mul2(a: i64, b: i64) -> (result: i64) {\n    return a * b;\n}\nfn sub2(a: i64, b: i64) -> (result: i64) {\n    return a - b;\n}\nfn entry_nest(x: i64, a: i64, b: i64) -> (result: i64) {\n    return sub2(x, mul2(a, b));\n}\n",
    );
    let codes = errors(&ok);
    assert!(codes.is_empty(), "nested calls regressed: {codes:?}");
}
