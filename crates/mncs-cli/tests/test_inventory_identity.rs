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
