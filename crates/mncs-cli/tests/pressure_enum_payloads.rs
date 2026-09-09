//! ENG-PRESSURE-0007: enum payloads carry ordinary valid MNCS value types.
//!
//! Bare-sequence payloads were refused at elaboration (`MNE171`) and
//! cross-module record payloads failed body validation (`MNB063`/`MNB066`)
//! because payloads stored raw source spellings while record fields store
//! canonical identity spellings. Payloads now resolve through the same
//! `profile_type` + `canonical_value_type` path as record fields, and
//! qualified payload construction (`alias.Type.Variant { ... }`) parses to
//! a finite constructor with the same record-literal retry as two-segment
//! paths. The corpus pins construction, qualified construction,
//! projection, qualified pattern consumption, argument/result flow, and
//! nested aggregates on every executable backend; invalid payload types
//! still fail closed (`MNE171`/`MNE170`, asserted in
//! `payload_rejections_stay_closed`).

use std::process::Command;

use serde_json::Value;

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn library(name: &str) -> String {
    format!("{}/../../library/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn run_experiment(source: &str, backend: &str, corpus: &str) -> (Option<i32>, Value, String) {
    let output = binary()
        .args([
            "experiment",
            "run",
            source,
            "--backend",
            backend,
            "--corpus",
            corpus,
        ])
        .env("MNCS_LIBRARY_PATH", library(""))
        .output()
        .expect("run experiment");
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let value: Value = serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("experiment JSON ({stderr}): {error}"));
    (output.status.code(), value, stderr)
}

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

#[test]
fn enum_payloads_compose_across_modules_per_backend() {
    let source = example("source/pressure-payload.mncs");
    let corpus = example("execution/pressure-payload-corpus.json");
    for backend in EXECUTABLE_BACKENDS {
        let (code, result, stderr) = run_experiment(&source, backend, &corpus);
        assert_eq!(
            code,
            Some(0),
            "{backend}: unexpected exit; stderr={stderr}; result={result:#}"
        );
        let cases = result["cases"]
            .as_array()
            .unwrap_or_else(|| panic!("{backend}: missing cases; {result:#}"));
        assert_eq!(cases.len(), 6, "{backend}: case count");
        for case in cases {
            let id = case["case_id"].as_str().unwrap_or("?");
            assert_eq!(
                case["status"], "returned",
                "{backend} {id}: status {case:#}"
            );
            assert_eq!(
                case["expectation_met"], true,
                "{backend} {id}: logical value mismatch; returned={:#}",
                case["returned"]
            );
        }
        let overall = result["status"].as_str().unwrap_or("");
        assert!(
            overall == "PASS" || overall == "UNKNOWN",
            "{backend}: overall status {overall} is not PASS or UNKNOWN"
        );
    }
}

/// Invalid payloads fail closed at elaboration: unknown nominals and
/// over-bound sequences keep `MNE171`, duplicated payload fields keep
/// `MNE170`. A silent acceptance here would mean the widened universe
/// admits ill-typed variants.
#[test]
fn payload_rejections_stay_closed() {
    let cases = [
        (
            "unknown",
            "mncs 0.10;\nmodule test.payload.neg_unknown;\nenum E { Ok { m: nosuch.T }, Bad }\nfn probe() -> (result: i64) {\n    return 0;\n}\n",
            "MNE171",
        ),
        (
            "overbound",
            "mncs 0.10;\nmodule test.payload.neg_overbound;\nenum E { Ok { m: [i64; 1025] }, Bad }\nfn probe() -> (result: i64) {\n    return 0;\n}\n",
            "MNE171",
        ),
        (
            "dupfield",
            "mncs 0.10;\nmodule test.payload.neg_dupfield;\nenum E { Ok { x: i64, x: i64 }, Bad }\nfn probe() -> (result: i64) {\n    return 0;\n}\n",
            "MNE170",
        ),
    ];
    for (name, text, code) in cases {
        let dir = std::env::temp_dir().join(format!("mncs-pressure-payload-{name}"));
        std::fs::create_dir_all(&dir).expect("create workspace");
        let path = dir.join(format!("{name}.mncs"));
        std::fs::write(&path, text).expect("write case");
        let output = binary()
            .args(["source-study", &path.to_string_lossy()])
            .env("MNCS_LIBRARY_PATH", library(""))
            .output()
            .expect("run source-study");
        let result: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
        let codes: Vec<String> = result["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter(|d| d["severity"] == "error")
            .filter_map(|d| d["code"].as_str().map(str::to_owned))
            .collect();
        assert!(
            codes.iter().any(|candidate| candidate == code),
            "{name}: expected {code} in {codes:?}"
        );
    }
}

/// CP-0008 reconciliation: canonical payload identity must not collapse
/// distinct nominals. Two structurally identical enums (`A`, `B`) stay
/// non-interchangeable both in return position and inside a payload
/// field. A silent acceptance here would mean canonicalization erased
/// nominal identity.
#[test]
fn nominal_identity_does_not_collapse() {
    let cases = [
        (
            "return-position",
            "mncs 0.10;\nmodule test.payload.neg_nominal_return;\nenum A { Ok { x: u64 }, Bad }\nenum B { Ok { x: u64 }, Bad }\nfn probe(a: A) -> (result: B) {\n    return a;\n}\n",
            "MNE103",
        ),
        (
            "payload-position",
            "mncs 0.10;\nmodule test.payload.neg_nominal_payload;\nenum A { Ok { x: u64 }, Bad }\nenum B { Ok { x: u64 }, Bad }\nenum S { W { v: A }, Z }\nfn probe(b: B) -> (result: S) {\n    return S.W { v: b };\n}\n",
            "MNE117",
        ),
    ];
    for (name, text, code) in cases {
        let dir = std::env::temp_dir().join(format!("mncs-pressure-payload-{name}"));
        std::fs::create_dir_all(&dir).expect("create workspace");
        let path = dir.join(format!("{name}.mncs"));
        std::fs::write(&path, text).expect("write case");
        let output = binary()
            .args(["source-study", &path.to_string_lossy()])
            .env("MNCS_LIBRARY_PATH", library(""))
            .output()
            .expect("run source-study");
        let result: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
        let codes: Vec<String> = result["diagnostics"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter(|d| d["severity"] == "error")
            .filter_map(|d| d["code"].as_str().map(str::to_owned))
            .collect();
        assert!(
            codes.iter().any(|candidate| candidate == code),
            "{name}: expected {code} in {codes:?}"
        );
    }
}
