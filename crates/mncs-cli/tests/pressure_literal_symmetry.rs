//! WEB-P-005: integer literals adapt symmetrically in binary arithmetic.
//!
//! A literal on the left adapts to the right operand's concrete type
//! exactly as a right-side literal adapts to the left's, so `1000 +% x`
//! means what `x +% 1000` means on every executable backend. Genuinely
//! mixed non-literal widths stay refused (MNE119), and an out-of-range
//! literal for a byte operand stays refused.

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

fn study_diagnostics(source_text: &str) -> Vec<Value> {
    let dir = std::env::temp_dir().join(format!(
        "mncs-literal-symmetry-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("create workspace");
    let path = dir.join("probe.mncs");
    std::fs::write(&path, source_text).expect("write case");
    let output = binary()
        .args(["source-study", &path.to_string_lossy()])
        .output()
        .expect("run source-study");
    let result: Value = serde_json::from_slice(&output.stdout).expect("front-end JSON");
    result["diagnostics"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

const EXECUTABLE_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

/// Left-literal and right-literal forms return identical values on every
/// backend, including the wrapping/saturating spellings from the pressure.
#[test]
fn literal_operands_adapt_symmetrically_on_every_backend() {
    let source = example("source/pressure-literal-symmetry.mncs");
    let corpus = example("execution/pressure-literal-symmetry-corpus.json");
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
        assert_eq!(cases.len(), 10, "{backend}: case count");
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
    }
}

/// Integer literals adapt to binary64 exactly (`|v| <= 2^53`) on every
/// backend, in both operand positions, and exponent notation parses to
/// correctly-rounded values — including magnitudes and subnormals no
/// dotted spelling can reach. (P-008; the inexact case stays refused
/// below.)
#[test]
fn integer_literals_adapt_to_float_exactly_on_every_backend() {
    let dir = std::env::temp_dir().join(format!(
        "mncs-float-adapt-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create workspace");
    std::fs::write(
        dir.join("adapt.mncs"),
        "mncs 0.16;\n\nmodule probe.float_adapt;\n\nfn half_sum<N: Nat>(xs: [f64; N]) -> (result: f64) {\n    iterate i over xs carrying acc: f64 = 0.0 {\n        next acc = acc + xs[i] * 2;\n    }\n    return acc;\n}\n\nfn twice(x: f64) -> (result: f64) {\n    return 2 * x;\n}\n\nfn big() -> (result: f64) {\n    return 1e300 + 1e300;\n}\n\nfn tiny() -> (result: f64) {\n    return 5e-324 + 0.0;\n}\n\nfn sci() -> (result: f64) {\n    return 1.5e-3 * 2;\n}\n",
    )
    .expect("write case");
    let float = |bits: u64| serde_json::json!({"float": {"bits": bits, "type": {"bits": 64}}});
    let seq = |lanes: Vec<serde_json::Value>| serde_json::json!({"sequence": {"values": lanes}});
    let request = |function: &str, arguments: Vec<serde_json::Value>| {
        let mut request = serde_json::json!({
            "schema_version": "0.1",
            "target": {"module": "probe.float_adapt", "function": function},
            "arguments": arguments,
            "step_budget": 8192,
        });
        if function == "half_sum" {
            request["type_arguments"] = serde_json::json!([{"kind": "nat", "value": 3}]);
        }
        request
    };
    // 1.0, 2.0, 3.0 doubled and summed; 21.0 doubled; 2e300;
    // subnormal 5e-324; 0.003. All hand-computed bit patterns.
    let corpus = serde_json::json!({
        "schema_version": "0.2",
        "name": "float-adapt",
        "cases": [
            {
                "id": "right-literal",
                "request": request("half_sum", vec![seq(vec![
                    float(4607182418800017408),
                    float(4611686018427387904),
                    float(4613937818241073152),
                ])]),
                "expected": [float(4622945017495814144)],
                "expected_status": "returned"
            },
            {
                "id": "left-literal",
                "request": request("twice", vec![float(4626604192193052672)]),
                "expected": [float(4631107791820423168)],
                "expected_status": "returned"
            },
            {
                "id": "exponent-big",
                "request": request("big", vec![]),
                "expected": [float(9099492520756278684)],
                "expected_status": "returned"
            },
            {
                "id": "exponent-subnormal",
                "request": request("tiny", vec![]),
                "expected": [float(1)],
                "expected_status": "returned"
            },
            {
                "id": "exponent-sci",
                "request": request("sci", vec![]),
                "expected": [float(4569063951553953530)],
                "expected_status": "returned"
            },
        ]
    });
    let corpus_path = dir.join("corpus.json");
    std::fs::write(&corpus_path, corpus.to_string()).expect("write corpus");
    let source = dir.join("adapt.mncs").to_string_lossy().into_owned();
    let corpus = corpus_path.to_string_lossy().into_owned();
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
        assert_eq!(cases.len(), 5, "{backend}: case count");
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
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// An integer literal the binary64 grid cannot hold stays refused
/// (MNE118 with an explicit-`as` remedy) instead of silently rounding,
/// and a non-finite exponent literal stays refused (MNP198).
#[test]
fn inexact_float_spellings_stay_closed() {
    // 2^53 + 1 in a binary64 position: not exactly representable.
    let wide = study_diagnostics(
        "mncs 0.16;\nmodule test.lit.wide;\nfn probe(x: f64) -> (result: f64) {\n    return x * 9007199254740993;\n}\n",
    );
    let wide_mne118: Vec<&Value> = wide.iter().filter(|d| d["code"] == "MNE118").collect();
    assert_eq!(
        wide_mne118.len(),
        1,
        "inexact literal must report exactly one MNE118, got: {wide:?}"
    );
    assert!(
        wide_mne118[0]["message"]
            .as_str()
            .is_some_and(
                |message| message.contains("not exactly representable") && message.contains("`as`")
            ),
        "MNE118 must name the loss and the remedy: {:?}",
        wide_mne118[0]["message"]
    );
    // The boundary itself (2^53) still adapts: no error diagnostics
    // (the honest float-finite obligation notice is not an error).
    let exact = study_diagnostics(
        "mncs 0.16;\nmodule test.lit.exact;\nfn probe(x: f64) -> (result: f64) {\n    return x * 9007199254740992;\n}\n",
    );
    let exact_errors: Vec<&Value> = exact.iter().filter(|d| d["severity"] == "error").collect();
    assert!(
        exact_errors.is_empty(),
        "2^53 must adapt cleanly, got: {exact:?}"
    );
    // Non-finite exponent magnitudes are refused at parse.
    let huge = study_diagnostics(
        "mncs 0.16;\nmodule test.lit.huge;\nfn probe() -> (result: f64) {\n    return 1e999;\n}\n",
    );
    assert!(
        huge.iter().any(|d| d["code"] == "MNP198"),
        "1e999 must report MNP198, got: {huge:?}"
    );
    // A negated exponent parses as one literal.
    let negated = study_diagnostics(
        "mncs 0.16;\nmodule test.lit.negexp;\nfn probe() -> (result: f64) {\n    return -1.5e-3;\n}\n",
    );
    let negated_errors: Vec<&Value> = negated
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert!(
        negated_errors.is_empty(),
        "-1.5e-3 must parse cleanly, got: {negated:?}"
    );
}

/// Genuinely mixed non-literal widths stay refused, and an out-of-range
/// literal for a byte operand stays refused: adaptation never invents a
/// width the literal cannot inhabit.
#[test]
fn non_adaptable_mismatches_stay_closed() {
    let mixed = study_diagnostics(
        "mncs 0.13;\nmodule test.lit.mixed;\nfn probe(a: i32, b: u64) -> (result: u64) {\n    return 1 + a + b;\n}\n",
    );
    assert!(
        mixed.iter().any(|d| d["code"] == "MNE119"),
        "variable mixed widths must stay MNE119, got: {mixed:?}"
    );
    let narrow = study_diagnostics(
        "mncs 0.13;\nmodule test.lit.narrow;\nfn probe(b: byte) -> (result: byte) {\n    return 300 & b;\n}\n",
    );
    assert!(
        narrow
            .iter()
            .any(|d| d["code"] == "MNE118" || d["code"] == "MNE119"),
        "out-of-range byte literal must stay refused, got: {narrow:?}"
    );
}
