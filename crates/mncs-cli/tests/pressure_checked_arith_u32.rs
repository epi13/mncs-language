//! P1-B01: unsigned checked arithmetic uses unsigned range tests on every backend.
//!
//! The C11 backend previously sign-extended narrow unsigned cells through
//! `(uint64_t)`, mistrapping every u32 operand or intermediate >= 2^31
//! (e.g. 255*16777216 = 4278190080 trapped on C11 while the other four
//! backends returned it). Narrow unsigned cells now reinterpret via
//! `(uint32_t)` before widening. The portable-WASM saturating path had the
//! companion gap (unsigned saturating refused past 31 bits); it now uses
//! unsigned comparisons with wrap-materialized saturation values.
//!
//! The corpus pins the boundary on all five backends: the store's
//! big-endian u32 decoder at max and small inputs, mid/mul-square products
//! that exceed INT32_MAX but fit u32, add at exactly max, the u32/i32
//! cross-boundary case (2147483647+1 is valid u32 and must not trap under
//! signed-32 logic), wrapping/saturating totals, a u64 control, and three
//! deterministic trap cases (u32 add/mul exactly one beyond max, i32 add
//! past signed max).

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

/// Range-discharge boundary at the study surface: the byte-fed decoder and
/// all literal-bounded helpers carry no open `integer-overflow` fact, while
/// every function over an unknown operand keeps exactly one per checked
/// operation. Loop-carried narrowing can never discharge: see
/// `range_loop_state_stays_open` below.
#[test]
fn range_discharge_pins_provable_decoder_and_open_unknowns() {
    let output = binary()
        .args([
            "source-study",
            &example("source/pressure-checked-arith-u32.mncs"),
        ])
        .env("MNCS_LIBRARY_PATH", library(""))
        .output()
        .expect("run source-study");
    let result: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
    let unresolved: Vec<String> = result["unresolved_obligations"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect();
    // Exactly the six unknown-operand checked operations; every
    // byte-derived and literal operation discharges through
    // `language-range-arithmetic-sound`.
    assert_eq!(unresolved.len(), 6, "refusal set: {unresolved:?}");
    assert!(
        unresolved
            .iter()
            .all(|identity| identity.contains("integer-overflow")),
        "only integer-overflow facts stay open: {unresolved:?}"
    );
}

/// Adversarial companion: loop-carried state has no narrow range fact, so a
/// checked accumulation over it stays open. Here `total` starts at literal
/// zero but 1024 iterations of `+ 16777216` reach 2^34, far past
/// `u32::MAX` — the analysis must not seed the state from its initializer
/// (a `[0, 0]` seed would compute the first-iteration interval
/// `[16777216, 16777216]`, discharge unsoundly, and bless an overflowing
/// loop). Loop state arrives as a block parameter pinned to its full
/// declared bounds; unknown operands widen rather than discharge.
/// The only other open fact is the standard iteration cost fact.
#[test]
fn range_loop_state_stays_open() {
    let source = "mncs 0.14;\nmodule test.range.loop;\nfn accumulate() -> (result: u32) {\n    iterate i up_to 1024 carrying total: u32 = 0 {\n        next total = total + 16777216;\n    }\n    return total;\n}\n";
    let dir = std::env::temp_dir().join(format!(
        "mncs-range-loop-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    std::fs::create_dir_all(&dir).expect("create workspace");
    let path = dir.join("probe.mncs");
    std::fs::write(&path, source).expect("write case");
    let output = binary()
        .args(["source-study", &path.to_string_lossy()])
        .env("MNCS_LIBRARY_PATH", library(""))
        .output()
        .expect("run source-study");
    let result: Value = serde_json::from_slice(&output.stdout).expect("study JSON");
    let unresolved: Vec<String> = result["unresolved_obligations"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|value| value.as_str().map(str::to_owned))
        .collect();
    assert_eq!(
        unresolved.len(),
        2,
        "accumulation + cost facts: {unresolved:?}"
    );
    assert_eq!(
        unresolved
            .iter()
            .filter(|identity| identity.contains("integer-overflow"))
            .count(),
        1,
        "loop-carried checked accumulation must stay open: {unresolved:?}"
    );
}

/// Value and trap cases agree on every backend: no backend may apply signed
/// overflow logic to unsigned values, and overflow exactly one beyond the
/// boundary traps deterministically everywhere.
#[test]
fn unsigned_checked_arith_agrees_on_every_backend() {
    let source = example("source/pressure-checked-arith-u32.mncs");
    let corpus = example("execution/pressure-checked-arith-u32-corpus.json");
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
        assert_eq!(cases.len(), 15, "{backend}: case count");
        for case in cases {
            let id = case["case_id"].as_str().unwrap_or("?");
            if id.starts_with("trap-") {
                assert_eq!(
                    case["status"], "runtime_failure",
                    "{backend} {id}: overflow one beyond max must trap; case={case:#}"
                );
            } else {
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
}
