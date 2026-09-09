//! MNCS-native JIT orchestration corpora (`library/jit/`, `docs/jit-architecture.md`).
//!
//! The JIT session/generation/binding/invalidation/planning/
//! profiling/proof/lifecycle logic is implemented in MNCS itself and
//! must agree across providers: the Cranelift backend executes the
//! same orchestration code as the reference layers. Small modules run
//! the full five-backend matrix; the two heavy session modules run
//! the bytecode + Cranelift pair the delivery architecture actually
//! uses (orchestration on the reference provider, definitions on the
//! native provider), plus the layered agreement check that covers the
//! reference executors.

use std::process::Command;

use serde_json::Value;

fn library_dir() -> String {
    format!("{}/../../library", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn run(source: &str, corpus: &str, backend: &str) -> Value {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "experiment",
            "run",
            source,
            "--backend",
            backend,
            "--corpus",
            corpus,
        ])
        .output()
        .expect("run experiment");
    assert!(
        output.status.success(),
        "{backend} {source}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("result JSON")
}

fn assert_all_met(result: &Value, backend: &str, expected_cases: usize) {
    let cases = result["cases"].as_array().unwrap();
    assert_eq!(cases.len(), expected_cases, "{backend}: corpus drift");
    for case in cases {
        assert_eq!(case["status"], "returned", "{backend}: {case}");
        assert_eq!(case["expectation_met"], true, "{backend}: {case}");
    }
}

fn check_layers(source: &str, corpus: &str) {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(["check-backend-execution", source, corpus])
        .output()
        .expect("run layered check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: Value = serde_json::from_slice(&output.stdout).expect("layered JSON");
    assert_eq!(result["status"], "consistent_over_corpus");
    assert_eq!(result["mismatching_cases"], 0);
}

const SMALL_MODULES: [(&str, &str, usize); 6] = [
    ("jit/types.mncs", "execution/jit-types-corpus.json", 4),
    ("jit/depends.mncs", "execution/jit-depends-corpus.json", 5),
    (
        "jit/lifecycle.mncs",
        "execution/jit-lifecycle-corpus.json",
        3,
    ),
    ("jit/plan.mncs", "execution/jit-plan-corpus.json", 4),
    ("jit/profile.mncs", "execution/jit-profile-corpus.json", 3),
    ("jit/proof.mncs", "execution/jit-proof-corpus.json", 3),
];

const FULL_BACKENDS: [&str; 5] = [
    "mncs-research-bytecode",
    "mncs-portable-wasm-mvp",
    "mncs-c11",
    "mncs-llvm-ir",
    "mncs-cranelift",
];

/// Vocabulary, dependency, lifecycle, planning, profiling, and proof
/// tables agree on every executable backend.
#[test]
fn jit_small_modules_agree_on_every_executable_backend() {
    for (module, corpus, cases) in SMALL_MODULES {
        let source = format!("{}/{}", library_dir(), module);
        let corpus = example(corpus);
        for backend in FULL_BACKENDS {
            let result = run(&source, &corpus, backend);
            assert_all_met(&result, backend, cases);
        }
    }
}

/// Session and binding flows agree on the bytecode + Cranelift pair.
#[test]
fn jit_session_modules_agree_on_bytecode_and_cranelift() {
    for (module, corpus, cases) in [
        ("jit/session.mncs", "execution/jit-session-corpus.json", 4),
        ("jit/binding.mncs", "execution/jit-binding-corpus.json", 6),
    ] {
        let source = format!("{}/{}", library_dir(), module);
        let corpus = example(corpus);
        for backend in ["mncs-research-bytecode", "mncs-cranelift"] {
            let result = run(&source, &corpus, backend);
            assert_all_met(&result, backend, cases);
        }
    }
}

/// Every JIT corpus agrees across the layered reference executors.
#[test]
fn jit_corpora_agree_across_layers() {
    for (module, corpus, _) in SMALL_MODULES {
        check_layers(&format!("{}/{}", library_dir(), module), &example(corpus));
    }
    for corpus in [
        "execution/jit-session-corpus.json",
        "execution/jit-binding-corpus.json",
    ] {
        let source = if corpus.contains("session") {
            format!("{}/jit/session.mncs", library_dir())
        } else {
            format!("{}/jit/binding.mncs", library_dir())
        };
        check_layers(&source, &example(corpus));
    }
}
