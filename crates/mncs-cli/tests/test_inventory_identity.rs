use std::{
    env, fs,
    path::PathBuf,
    process::{Command, Output},
};

fn run_inventory(binary: &PathBuf, source: &PathBuf) -> serde_json::Value {
    let output: Output = Command::new(binary)
        .arg("test-inventory")
        .arg(source)
        .output()
        .expect("test-inventory command should start");
    assert!(
        output.status.success(),
        "test-inventory failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("test-inventory should emit JSON")
}

fn run_declaration_inventory(binary: &PathBuf, source: &PathBuf) -> serde_json::Value {
    let output = Command::new(binary)
        .arg("declaration-inventory")
        .arg(source)
        .output()
        .expect("declaration-inventory command should start");
    assert!(
        output.status.success(),
        "declaration-inventory failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("declaration-inventory should emit JSON")
}

#[test]
fn semantic_inventory_identity_is_independent_of_checkout_path() {
    let binary = PathBuf::from(
        env::var_os("CARGO_BIN_EXE_mncs").expect("Cargo should provide the mncs binary path"),
    );
    let root = env::temp_dir().join(format!("mncs-inventory-identity-{}", std::process::id()));
    let first = root.join("checkout-a/source.mncs");
    let second = root.join("checkout-b/source.mncs");
    let source =
        "mncs 0.17;\nmodule test.inventory_identity;\ntest one() -> (result: i64) { return 1; }\n";

    fs::create_dir_all(first.parent().expect("first parent")).expect("first directory");
    fs::create_dir_all(second.parent().expect("second parent")).expect("second directory");
    fs::write(&first, source).expect("first source");
    fs::write(&second, source).expect("second source");

    let first_document = run_inventory(&binary, &first);
    let second_document = run_inventory(&binary, &second);
    assert_eq!(
        first_document["inventory"], second_document["inventory"],
        "semantic inventory must not bind to an absolute checkout path"
    );

    fs::remove_dir_all(root).expect("temporary inventory sources should be removable");
}

#[test]
fn generic_declaration_inventory_is_the_callable_authority() {
    let binary = PathBuf::from(
        env::var_os("CARGO_BIN_EXE_mncs").expect("Cargo should provide the mncs binary path"),
    );
    let root = env::temp_dir().join(format!("mncs-declaration-inventory-{}", std::process::id()));
    let source = root.join("source.mncs");
    fs::create_dir_all(&root).expect("inventory directory should be created");
    fs::write(
        &source,
        "mncs 0.17;\nmodule test.declaration_inventory;\nfn helper() -> (result: i64) { return 1; }\ntest one() -> (result: i64) { return helper(); }\n",
    )
    .expect("inventory source should be written");

    let document = run_declaration_inventory(&binary, &source);
    assert_eq!(document["schema_version"], "mncs.declaration-inventory/1");
    assert!(document["valid"].as_bool().unwrap());
    let callables = document["inventory"]["callables"].as_array().unwrap();
    let helper = callables
        .iter()
        .find(|item| item["name"] == "helper")
        .unwrap();
    let test = callables.iter().find(|item| item["name"] == "one").unwrap();
    assert_eq!(helper["callable_kind"], "function");
    assert!(helper["test_case_identity"].is_null());
    assert_eq!(test["callable_kind"], "test");
    assert!(test["test_case_identity"]
        .as_str()
        .unwrap()
        .starts_with("mncs:"));
    assert!(callables
        .iter()
        .all(|item| !item["name"].as_str().unwrap().starts_with("elaborate_")));
    assert!(document["inventory"]["inventory_identity"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));

    fs::remove_dir_all(root).expect("temporary inventory sources should be removable");
}

#[test]
fn native_test_executes_the_compiler_test_identity_and_reports_its_callable_receipt() {
    let binary = PathBuf::from(
        env::var_os("CARGO_BIN_EXE_mncs").expect("Cargo should provide the mncs binary path"),
    );
    let root = env::temp_dir().join(format!(
        "mncs-test-identity-dispatch-{}",
        std::process::id()
    ));
    let source = root.join("source.mncs");
    let library = root.join("library");
    let assertions = library.join("mncs/test/assertions.mncs");
    let suite = library.join("mncs/test/suite.mncs");
    let result_path = root.join("result.json");
    fs::create_dir_all(assertions.parent().expect("assertion module parent"))
        .expect("identity test library should be created");
    fs::write(
        &assertions,
        r#"mncs 0.18;
module mncs.test.assertions;
enum Verdict { PASS, FAIL, SKIP, UNSUPPORTED }
enum FailureKind { NoFailure, Assertion, Setup, Compile, Runtime, Timeout, Unsupported, Infrastructure }
enum TestStatus { PASSED, FAILED, SKIPPED, UNSUPPORTED }
enum TestClassification { PASSED, TEST_FAILURE, UNSUPPORTED }
record TestResult {
    verdict: Verdict,
    verdict_code: u64,
    failure_kind: FailureKind,
    failure_code: u64,
    assertions: u64,
    failures: u64,
    expected: i64,
    actual: i64,
    assertion_code: u64
}
record TestProjection {
    status: TestStatus,
    classification: TestClassification,
    verdict: Verdict,
    failure_kind: FailureKind
}
fn project_test(test: TestResult) -> (result: TestProjection) {
    return TestProjection {
        status: TestStatus.PASSED,
        classification: TestClassification.PASSED,
        verdict: test.verdict,
        failure_kind: test.failure_kind
    };
}
"#,
    )
    .expect("assertion test module should be written");
    fs::write(
        &suite,
        r#"mncs 0.18;
module mncs.test.suite;
use mncs.test.assertions;
record SuiteSummary {
    verdict: Verdict,
    verdict_code: u64,
    total: u64,
    passed: u64,
    failed: u64,
    skipped: u64,
    unsupported: u64,
    assertion_failures: u64,
    setup_failures: u64,
    compile_failures: u64,
    runtime_failures: u64,
    timeout_failures: u64,
    infrastructure_failures: u64,
    assertion_count: u64,
    failure_count: u64
}
enum SuiteStatus { PASSED, FAILED, UNSUPPORTED }
enum SuiteClassification { PASSED, TEST_FAILURE, UNSUPPORTED }
record SuiteProjection {
    status: SuiteStatus,
    classification: SuiteClassification,
    verdict: Verdict
}
fn empty() -> (result: SuiteSummary) {
    return SuiteSummary {
        verdict: Verdict.PASS, verdict_code: 0, total: 0, passed: 0,
        failed: 0, skipped: 0, unsupported: 0, assertion_failures: 0,
        setup_failures: 0, compile_failures: 0, runtime_failures: 0,
        timeout_failures: 0, infrastructure_failures: 0,
        assertion_count: 0, failure_count: 0
    };
}
fn observe(summary: SuiteSummary, test: TestResult) -> (result: SuiteSummary) {
    return SuiteSummary {
        ..summary,
        total: summary.total +% 1,
        passed: summary.passed +% 1,
        assertion_count: summary.assertion_count +% test.assertions,
        failure_count: summary.failure_count +% test.failures
    };
}
fn project(summary: SuiteSummary) -> (result: SuiteProjection) {
    return SuiteProjection {
        status: SuiteStatus.PASSED,
        classification: SuiteClassification.PASSED,
        verdict: summary.verdict
    };
}
"#,
    )
    .expect("suite test module should be written");
    fs::write(
        &source,
        r#"mncs 0.18;
module test.identity_dispatch;
use mncs.test.assertions;
use mncs.test.suite;
test selected_identity() -> (result: TestResult) {
    return TestResult {
        verdict: Verdict.PASS,
        verdict_code: 0,
        failure_kind: FailureKind.NoFailure,
        failure_code: 0,
        assertions: 1,
        failures: 0,
        expected: 42,
        actual: 42,
        assertion_code: 9001
    };
}
"#,
    )
    .expect("identity test source should be written");

    let inventory_output = Command::new(&binary)
        .arg("test-inventory")
        .arg(&source)
        .env("MNCS_LIBRARY_PATH", &library)
        .output()
        .expect("test-inventory command should start");
    assert!(
        inventory_output.status.success(),
        "test-inventory failed: {}",
        String::from_utf8_lossy(&inventory_output.stderr)
    );
    let inventory: serde_json::Value =
        serde_json::from_slice(&inventory_output.stdout).expect("test-inventory should emit JSON");
    let identity = inventory["inventory"]["tests"][0]["test_case_identity"]
        .as_str()
        .expect("compiler should issue a test identity")
        .to_owned();
    let output = Command::new(&binary)
        .arg("test")
        .arg(&source)
        .arg("--library")
        .arg(&library)
        .arg("--test-identity")
        .arg(&identity)
        .arg("--result")
        .arg(&result_path)
        .output()
        .expect("native test command should start");
    assert!(
        output.status.success(),
        "native test command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(&result_path).expect("test result should be written"))
            .expect("test result should be valid JSON");
    let test = &result["tests"][0];
    let semantic = &test["semantic"];
    let invocation = &test["callable_invocation"];
    assert_eq!(result["verdict"], "PASS");
    assert_eq!(semantic["test_case_identity"], identity);
    assert_eq!(invocation["test_case_identity"], identity);
    assert_eq!(
        invocation["callable_identity"],
        semantic["function_identity"]
    );
    assert_eq!(
        invocation["declaration_identity"],
        semantic["declaration_identity"]
    );
    assert_eq!(
        invocation["signature_identity"],
        semantic["signature_identity"]
    );
    assert_eq!(
        invocation["artifact_identity"],
        result["execution"]["artifact_identity"]
    );
    assert_eq!(test["execution"]["invoked_callable"], invocation.clone());
    assert!(semantic["signature_identity"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));

    fs::remove_dir_all(root).expect("identity test fixtures should be removable");
}

#[test]
fn language_inventory_uses_explicit_public_intrinsics() {
    let binary = PathBuf::from(
        env::var_os("CARGO_BIN_EXE_mncs").expect("Cargo should provide the mncs binary path"),
    );
    let output = Command::new(binary)
        .arg("language-inventory")
        .output()
        .expect("language-inventory command should start");
    assert!(output.status.success());
    let document: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("language-inventory should emit JSON");
    assert_eq!(document["schema_version"], "mncs.language-inventory/1");
    assert!(document["intrinsics"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| !item["name"].as_str().unwrap().starts_with("elaborate_")));
    assert!(document["inventory_identity"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
}

#[test]
fn native_test_prunes_unreachable_functions_while_preserving_identities_and_generics() {
    let binary = PathBuf::from(
        env::var_os("CARGO_BIN_EXE_mncs").expect("Cargo should provide the mncs binary path"),
    );
    let root = env::temp_dir().join(format!(
        "mncs-test-closure-pruning-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let source = root.join("suite.mncs");
    let library = root.join("library");
    let assertions = library.join("mncs/test/assertions.mncs");
    let suite = library.join("mncs/test/suite.mncs");
    let helpers = library.join("demo/helpers.mncs");
    let result_path = root.join("result.json");
    fs::create_dir_all(assertions.parent().expect("assertion dir")).expect("assertion dir");
    fs::create_dir_all(helpers.parent().expect("helpers dir")).expect("helpers dir");
    fs::write(
        &assertions,
        r#"mncs 0.18;
module mncs.test.assertions;
enum Verdict { PASS, FAIL, SKIP, UNSUPPORTED }
enum FailureKind { NoFailure, Assertion, Setup, Compile, Runtime, Timeout, Unsupported, Infrastructure }
enum TestStatus { PASSED, FAILED, SKIPPED, UNSUPPORTED }
enum TestClassification { PASSED, TEST_FAILURE, UNSUPPORTED }
record TestResult {
    verdict: Verdict,
    verdict_code: u64,
    failure_kind: FailureKind,
    failure_code: u64,
    assertions: u64,
    failures: u64,
    expected: i64,
    actual: i64,
    assertion_code: u64
}
record TestProjection {
    status: TestStatus,
    classification: TestClassification,
    verdict: Verdict,
    failed: bool,
    skipped: bool,
    unsupported: bool
}
fn pass() -> (result: TestResult) {
    return TestResult {
        verdict: Verdict.PASS, verdict_code: 0,
        failure_kind: FailureKind.NoFailure, failure_code: 0,
        assertions: 1, failures: 0, expected: 1, actual: 1, assertion_code: 0
    };
}
fn fail_code(code: u64) -> (result: TestResult) {
    return TestResult {
        verdict: Verdict.FAIL, verdict_code: 1,
        failure_kind: FailureKind.Assertion, failure_code: 1,
        assertions: 1, failures: 1, expected: 1, actual: 0, assertion_code: code
    };
}
fn project_test(result: TestResult) -> (projection: TestProjection) {
    return match result.verdict {
        PASS => TestProjection { status: TestStatus.PASSED, classification: TestClassification.PASSED, verdict: Verdict.PASS, failed: false, skipped: false, unsupported: false },
        FAIL => TestProjection { status: TestStatus.FAILED, classification: TestClassification.TEST_FAILURE, verdict: Verdict.FAIL, failed: true, skipped: false, unsupported: false },
        SKIP => TestProjection { status: TestStatus.SKIPPED, classification: TestClassification.PASSED, verdict: Verdict.SKIP, failed: false, skipped: true, unsupported: false },
        UNSUPPORTED => TestProjection { status: TestStatus.UNSUPPORTED, classification: TestClassification.UNSUPPORTED, verdict: Verdict.UNSUPPORTED, failed: false, skipped: false, unsupported: true }
    };
}
"#,
    )
    .expect("assertions");
    fs::write(
        &suite,
        r#"mncs 0.18;
module mncs.test.suite;
use mncs.test.assertions;
record SuiteSummary {
    verdict: Verdict, verdict_code: u64,
    total: u64, passed: u64, failed: u64, skipped: u64, unsupported: u64
}
record SuiteProjection {
    verdict: Verdict,
    classification: TestClassification,
    failed: bool,
    unsupported: bool
}
fn empty() -> (summary: SuiteSummary) {
    return SuiteSummary { verdict: Verdict.PASS, verdict_code: 0, total: 0, passed: 0, failed: 0, skipped: 0, unsupported: 0 };
}
fn observe(summary: SuiteSummary, item: TestResult) -> (next_summary: SuiteSummary) {
    let is_fail: bool = item.verdict_code != 0;
    return SuiteSummary {
        verdict: Verdict.PASS,
        verdict_code: summary.verdict_code + select(is_fail, 1, 0),
        total: summary.total + 1,
        passed: summary.passed + select(is_fail, 0, 1),
        failed: summary.failed + select(is_fail, 1, 0),
        skipped: summary.skipped,
        unsupported: summary.unsupported
    };
}
fn project(summary: SuiteSummary) -> (projection: SuiteProjection) {
    if summary.failed > 0 {
        return SuiteProjection { verdict: Verdict.FAIL, classification: TestClassification.TEST_FAILURE, failed: true, unsupported: false };
    }
    return SuiteProjection { verdict: Verdict.PASS, classification: TestClassification.PASSED, failed: false, unsupported: false };
}
"#,
    )
    .expect("suite");
    fs::write(
        &helpers,
        r#"mncs 0.18;
module demo.helpers;
fn reached_generic<N: Nat>(buf: [byte; up_to N]) -> (result: u64) {
    return buf.len;
}
fn unused_dead_1(x: u64) -> (result: u64) { return x + 1; }
fn unused_dead_2(x: u64) -> (result: u64) { return unused_dead_1(x) + 2; }
fn unused_dead_3(x: u64) -> (result: u64) { return unused_dead_2(x) + 3; }
fn unused_dead_4(x: u64) -> (result: u64) { return unused_dead_3(x) + 4; }
fn unused_dead_generic<N: Nat>(buf: [byte; up_to N]) -> (result: u64) { return buf.len + 99; }
"#,
    )
    .expect("helpers");
    fs::write(
        &source,
        r#"mncs 0.18;
module tests.closure_pruning;
use demo.helpers as h;
use mncs.test.assertions;
use mncs.test.suite;

test uses_generic_helper() -> (result: TestResult) {
    let b: [byte; up_to 4] = [1, 2, 3];
    if h.reached_generic<4>(b) == 3 {
        return pass();
    }
    return fail_code(1);
}

test unselected_test() -> (result: TestResult) {
    return pass();
}
"#,
    )
    .expect("source");

    let output = Command::new(&binary)
        .arg("test")
        .arg(&source)
        .arg("--library")
        .arg(&library)
        .arg("--filter")
        .arg("uses_generic_helper")
        .arg("--result")
        .arg(&result_path)
        .output()
        .expect("native test command should run");
    assert!(
        output.status.success(),
        "native test command failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value =
        serde_json::from_slice(&fs::read(&result_path).expect("result file"))
            .expect("valid JSON");
    assert_eq!(result["verdict"], "PASS");
    assert_eq!(result["selection"]["available_test_count"], 2);
    assert_eq!(result["selection"]["selected_test_count"], 1);
    let original = result["execution"]["original_function_count"]
        .as_u64()
        .expect("original_function_count");
    let compiled = result["execution"]["compiled_function_count"]
        .as_u64()
        .expect("compiled_function_count");
    assert!(
        compiled < original,
        "expected unreachable functions to be pruned: compiled={compiled} original={original}"
    );
    assert!(
        result["execution"]["artifact_bytes"].as_u64().unwrap_or(0) > 0,
        "artifact_bytes should be reported"
    );

    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn native_test_module_elaboration_cache_reuses_invalidates_and_refreshes_spans() {
    let binary = PathBuf::from(
        env::var_os("CARGO_BIN_EXE_mncs").expect("Cargo should provide the mncs binary path"),
    );
    let root = env::temp_dir().join(format!(
        "mncs-test-mod-elab-cache-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    let cache_dir = root.join("elab-cache");
    let source = root.join("suite.mncs");
    let library = root.join("library");
    let assertions = library.join("mncs/test/assertions.mncs");
    let suite = library.join("mncs/test/suite.mncs");
    let leaf = library.join("demo/leaf.mncs");
    let mid = library.join("demo/mid.mncs");
    let result_path = root.join("result.json");
    fs::create_dir_all(assertions.parent().expect("assertion dir")).expect("assertion dir");
    fs::create_dir_all(leaf.parent().expect("demo dir")).expect("demo dir");
    fs::write(
        &assertions,
        r#"mncs 0.18;
module mncs.test.assertions;
enum Verdict { PASS, FAIL, SKIP, UNSUPPORTED }
enum FailureKind { NoFailure, Assertion, Setup, Compile, Runtime, Timeout, Unsupported, Infrastructure }
enum TestStatus { PASSED, FAILED, SKIPPED, UNSUPPORTED }
enum TestClassification { PASSED, TEST_FAILURE, UNSUPPORTED }
record TestResult { verdict: Verdict, assertions: u64, failures: u64, failure_kind: FailureKind, failure_code: u64, expected: u64, actual: u64 }
record TestProjection { verdict: Verdict, status: TestStatus, classification: TestClassification, failure_kind: FailureKind }
fn pass_result(v: u64) -> (result: TestResult) {
    return TestResult { verdict: Verdict.PASS, assertions: 1, failures: 0, failure_kind: FailureKind.NoFailure, failure_code: 0, expected: v, actual: v };
}
fn fail_result(e: u64, a: u64) -> (result: TestResult) {
    return TestResult { verdict: Verdict.FAIL, assertions: 1, failures: 1, failure_kind: FailureKind.Assertion, failure_code: 1, expected: e, actual: a };
}
fn equals_u64(e: u64, a: u64) -> (result: TestResult) {
    if e == a { return pass_result(e); }
    return fail_result(e, a);
}
fn project_test(r: TestResult) -> (result: TestProjection) {
    return match r.verdict {
        Verdict.PASS => TestProjection { verdict: Verdict.PASS, status: TestStatus.PASSED, classification: TestClassification.PASSED, failure_kind: FailureKind.NoFailure },
        Verdict.FAIL => TestProjection { verdict: Verdict.FAIL, status: TestStatus.FAILED, classification: TestClassification.TEST_FAILURE, failure_kind: r.failure_kind },
        Verdict.SKIP => TestProjection { verdict: Verdict.SKIP, status: TestStatus.SKIPPED, classification: TestClassification.PASSED, failure_kind: FailureKind.NoFailure },
        Verdict.UNSUPPORTED => TestProjection { verdict: Verdict.UNSUPPORTED, status: TestStatus.UNSUPPORTED, classification: TestClassification.UNSUPPORTED, failure_kind: FailureKind.Unsupported }
    };
}
"#,
    )
    .expect("assertions");
    fs::write(
        &suite,
        r#"mncs 0.18;
module mncs.test.suite;
use mncs.test.assertions;
record SuiteSummary { verdict: Verdict, total: u64, passed: u64, failed: u64, skipped: u64, unsupported: u64 }
record SuiteProjection { verdict: Verdict, classification: TestClassification }
fn empty() -> (result: SuiteSummary) {
    return SuiteSummary { verdict: Verdict.PASS, total: 0, passed: 0, failed: 0, skipped: 0, unsupported: 0 };
}
fn observe(s: SuiteSummary, r: TestResult) -> (result: SuiteSummary) {
    return match r.verdict {
        Verdict.PASS => SuiteSummary { verdict: s.verdict, total: s.total + 1, passed: s.passed + 1, failed: s.failed, skipped: s.skipped, unsupported: s.unsupported },
        Verdict.FAIL => SuiteSummary { verdict: Verdict.FAIL, total: s.total + 1, passed: s.passed, failed: s.failed + 1, skipped: s.skipped, unsupported: s.unsupported },
        Verdict.SKIP => SuiteSummary { verdict: s.verdict, total: s.total + 1, passed: s.passed, failed: s.failed, skipped: s.skipped + 1, unsupported: s.unsupported },
        Verdict.UNSUPPORTED => SuiteSummary { verdict: Verdict.UNSUPPORTED, total: s.total + 1, passed: s.passed, failed: s.failed, skipped: s.skipped, unsupported: s.unsupported + 1 }
    };
}
fn project(s: SuiteSummary) -> (result: SuiteProjection) {
    if s.failed > 0 { return SuiteProjection { verdict: Verdict.FAIL, classification: TestClassification.TEST_FAILURE }; }
    return SuiteProjection { verdict: Verdict.PASS, classification: TestClassification.PASSED };
}
"#,
    )
    .expect("suite");
    fs::write(
        &leaf,
        r#"mncs 0.18;
module demo.leaf;
fn compute(x: u64) -> (result: u64) { return x +% 7; }
"#,
    )
    .expect("leaf");
    fs::write(
        &mid,
        r#"mncs 0.18;
module demo.mid;
use demo.leaf as leaf;
fn forward(x: u64) -> (result: u64) { return leaf.compute(x); }
"#,
    )
    .expect("mid");
    let base_suite = r#"mncs 0.18;
module tests.elab_cache_witness;
use demo.mid as mid;
use mncs.test.assertions;
use mncs.test.suite;

test checks_mid_value() -> (result: TestResult) {
    return equals_u64(10, mid.forward(3));
}
"#;
    fs::write(&source, base_suite).expect("source");

    let run_cached = |expect_ok: bool| -> serde_json::Value {
        let output = Command::new(&binary)
            .arg("test")
            .arg(&source)
            .arg("--library")
            .arg(&library)
            .arg("--elaboration-cache-dir")
            .arg(&cache_dir)
            .arg("--result")
            .arg(&result_path)
            .output()
            .expect("mncs test should run");
        if expect_ok {
            assert!(
                output.status.success(),
                "expected success: stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        serde_json::from_slice(&fs::read(&result_path).expect("result json"))
            .expect("valid result json")
    };

    // 1. Cold run: links and publishes all imported modules.
    let cold = run_cached(true);
    assert_eq!(cold["verdict"], "PASS");
    let cold_line = cold["tests"][0]["source_span"]["line"].as_u64().unwrap();
    assert_eq!(cold_line, 7);
    let cold_obs = &cold["execution"]["module_elaboration_cache"];
    assert_eq!(cold_obs["reused_count"].as_u64(), Some(0));
    assert!(cold_obs["linked_count"].as_u64().unwrap() >= 4, "{cold_obs}");
    assert!(cold_obs["published_entry_count"].as_u64().unwrap() >= 4, "{cold_obs}");

    // 2. Warm run after inserting 3 comment lines at top of root suite:
    //    reuses ALL imported modules and reports fresh source_span line 10!
    fs::write(&source, format!("// line 1\n// line 2\n// line 3\n{base_suite}")).expect("root edit");
    let warm = run_cached(true);
    assert_eq!(warm["verdict"], "PASS");
    assert_eq!(warm["tests"][0]["source_span"]["line"].as_u64(), Some(10));
    let warm_obs = &warm["execution"]["module_elaboration_cache"];
    assert_eq!(warm_obs["linked_count"].as_u64(), Some(0), "{warm_obs}");
    assert!(warm_obs["reused_count"].as_u64().unwrap() >= 4, "{warm_obs}");

    // 3. Imported-body edit in demo.leaf (7 -> 8, identical signature):
    //    demo.leaf relinks, demo.mid is reused with refreshed leaf body, and test FAILS (actual 11 != 10)!
    fs::write(
        &leaf,
        r#"mncs 0.18;
module demo.leaf;
fn compute(x: u64) -> (result: u64) { return x +% 8; }
"#,
    )
    .expect("leaf body edit");
    let body_edit = run_cached(false);
    assert_eq!(body_edit["verdict"], "FAIL", "{body_edit}");
    assert_eq!(body_edit["tests"][0]["native_result"]["actual"].as_u64(), Some(11));
    let body_obs = &body_edit["execution"]["module_elaboration_cache"];
    let linked: Vec<&str> = body_obs["linked_module_subjects"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    let reused: Vec<&str> = body_obs["reused_module_subjects"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(linked.contains(&"demo.leaf"), "demo.leaf must relink: {body_obs}");
    assert!(reused.contains(&"demo.mid"), "demo.mid should reuse when leaf interface unchanged: {body_obs}");

    // 4. Imported-interface edit in demo.leaf (return type u64 -> bool) + shifted line:
    //    invalidates BOTH demo.leaf and downstream demo.mid, failing compilation with fresh diagnostic!
    fs::write(
        &leaf,
        r#"// extra comment 1
// extra comment 2
mncs 0.18;
module demo.leaf;
fn compute(x: u64) -> (result: bool) { return x == 0; }
"#,
    )
    .expect("leaf interface edit");
    let iface_edit = run_cached(false);
    assert_eq!(iface_edit["verdict"], "UNKNOWN", "{iface_edit}");
    assert_eq!(iface_edit["classification"], "compile_failure", "{iface_edit}");

    // 5. Restore demo.leaf -> PASS again with fresh spans.
    fs::write(
        &leaf,
        r#"mncs 0.18;
module demo.leaf;
fn compute(x: u64) -> (result: u64) { return x +% 7; }
"#,
    )
    .expect("leaf restore");
    let restored = run_cached(true);
    assert_eq!(restored["verdict"], "PASS");
    assert_eq!(restored["tests"][0]["source_span"]["line"].as_u64(), Some(10));

    fs::remove_dir_all(root).expect("cleanup");
}
