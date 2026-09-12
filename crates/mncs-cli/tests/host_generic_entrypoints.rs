//! P1-013 / P2-003: host invocation of generic entrypoints.
//!
//! A host selects a concrete specialization with explicit
//! `type_arguments` on the execution request — no hand-written wrapper
//! functions. The corpus names Nat arguments (`{"kind": "nat", "value":
//! 8}`), multi-parameter Nat addresses (`pick2_sum<2, 2>` and
//! `<3, 2>`, pinning positional spelling round-trips), type arguments
//! (`{"kind": "type", "type": "i64"}`), view bounds, cross-module
//! declarations, and nominal record arguments; elaboration compiles
//! each named instantiation through the same specialization queue,
//! identity scheme, and ceiling sweep as in-language calls, and every
//! executable backend lowers and serves it. `probe_same_*` wrappers
//! instantiate the same arguments in-language, pinning the
//! single-specialization contract: the seeded artifact carries exactly
//! one entrypoint row per instantiation however it was requested.
//!
//! Malformed instantiations fail at elaboration with the same MNE codes
//! an in-language mistake reports (MNE131/MNE221/MNE222/MNE225/MNE105);
//! a bare generic target fails closed at execution with an explicit
//! `requires explicit type_arguments` refusal on every layer; artifacts
//! that predate the entrypoint map fail closed with a recompile
//! diagnostic instead of mislinking.

use std::process::Command;

use serde_json::{json, Value};

fn example(name: &str) -> String {
    format!("{}/../../examples/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn library_dir() -> String {
    format!("{}/../../library", env!("CARGO_MANIFEST_DIR"))
}

fn binary() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mncs"))
}

fn source() -> String {
    example("source/pressure-host-generics.mncs")
}

fn corpus() -> String {
    example("execution/pressure-host-generics-corpus.json")
}

fn workspace(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "mncs-host-generics-{tag}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create workspace");
    dir
}

fn run_experiment(backend: &str) -> Value {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(["experiment", "run", &source(), "--backend", backend])
        .arg("--corpus")
        .arg(corpus())
        .output()
        .expect("run host-generics experiment");
    assert!(
        output.status.success(),
        "{backend}: experiment exits 0: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("experiment JSON")
}

fn case_by_id<'a>(report: &'a Value, id: &str) -> &'a Value {
    report
        .get("cases")
        .and_then(Value::as_array)
        .and_then(|cases| {
            cases
                .iter()
                .find(|case| case.get("case_id").and_then(Value::as_str) == Some(id))
        })
        .unwrap_or_else(|| panic!("case {id} present: {report:#}"))
}

/// Every executable backend serves every host-named instantiation —
/// Nat, type, view-bound, cross-module, and nominal — with the
/// translation validator independently agreeing on each one.
#[test]
fn host_named_instantiations_pass_on_all_five_backends() {
    for backend in [
        "mncs-research-bytecode",
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let report = run_experiment(backend);
        assert_eq!(report["status"], "PASS", "{backend}: overall PASS");
        for id in [
            "host-nat-fill",
            "host-nat-fill-4",
            "host-nat-pair-22",
            "host-nat-pair-32",
            "host-type-first",
            "host-view-len",
            "host-xmod-fill",
            "host-xmod-first",
            "host-nominal-point",
            "host-nominal-point-identity",
            "same-fill",
            "same-first",
            "same-lib",
            "same-pick",
        ] {
            let case = case_by_id(&report, id);
            assert_eq!(case["status"], "returned", "{backend}: {id} returns");
            assert_eq!(
                case["status_met"], true,
                "{backend}: {id} meets expectation"
            );
        }
        let judgements = report
            .get("translation_validations")
            .and_then(Value::as_array)
            .map(|validations| {
                validations
                    .iter()
                    .map(|validation| {
                        validation
                            .get("judgement")
                            .and_then(Value::as_str)
                            .unwrap_or("missing")
                            .to_owned()
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        assert!(
            !judgements.is_empty(),
            "{backend}: translation validation ran"
        );
        for judgement in judgements {
            assert_eq!(judgement, "PASS", "{backend}: validation agrees");
        }
    }
}

/// One instantiation, however requested: the seeded artifact carries
/// exactly one entrypoint row per (declaration, arguments) — the
/// in-language `probe_same_*` wrappers and the host seeds deduplicate
/// through one specialization — and every row names its requesting
/// spellings.
#[test]
fn seeded_artifacts_carry_one_entrypoint_row_per_instantiation() {
    let dir = workspace("rows");
    let compiled = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "compile",
            &source(),
            "--emit",
            "backend",
            "--target",
            "mncs-c11",
        ])
        .arg("--corpus")
        .arg(corpus())
        .arg("--output-dir")
        .arg(&dir)
        .output()
        .expect("compile seeded artifact");
    assert!(
        compiled.status.success(),
        "seeded compile exits 0: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let artifact: Value = serde_json::from_slice(
        &std::fs::read(dir.join("backend.json")).expect("backend.json emitted"),
    )
    .expect("artifact JSON");
    assert_eq!(artifact["schema_version"], "0.4");
    let rows = artifact
        .get("generic_entrypoints")
        .and_then(Value::as_array)
        .expect("entrypoint map present");
    assert_eq!(rows.len(), 10, "one row per instantiation: {rows:?}");
    let rows_for = |module: &str, function: &str| {
        rows.iter()
            .filter(|row| {
                row.get("generic_module").and_then(Value::as_str) == Some(module)
                    && row.get("generic_function").and_then(Value::as_str) == Some(function)
            })
            .collect::<Vec<_>>()
    };
    // Requested both from the host and in-language: still one row, and
    // the row keeps the host spellings that address it.
    let fill8 = rows_for("pressure.host_generics", "grow_fill_local");
    assert_eq!(fill8.len(), 2, "W=8 and W=4 stay distinct: {fill8:?}");
    let w8 = fill8
        .iter()
        .find(|row| row["canonical_args"] == "value:8")
        .expect("W=8 row");
    assert_eq!(w8["args_spellings"], json!(["8"]));
    assert!(
        w8["entry_function"]
            .as_str()
            .is_some_and(|entry| entry.starts_with("grow_fill_local__spec_")),
        "deterministic entry name: {w8:?}"
    );
    let w4 = fill8
        .iter()
        .find(|row| row["canonical_args"] == "value:4")
        .expect("W=4 row");
    assert_eq!(w4["args_spellings"], json!(["4"]));
    assert_ne!(w8["entry_function"], w4["entry_function"]);
    // Cross-module and nominal rows resolve through the defining module.
    let xmod = rows_for("pressure.host_generics.lib", "grow_fill");
    assert_eq!(xmod.len(), 1, "{xmod:?}");
    assert_eq!(xmod[0]["args_spellings"], json!(["4"]));
    // Multi-parameter Nat addresses round-trip positionally: the
    // repeated spelling (2, 2) is not deduplicated to (2), the
    // unsorted spelling (3, 2) is not reordered to (2, 3), and the
    // host-seeded (2, 2) plus the in-language probe_same_pick share
    // one row (single specialization, first-seen address kept).
    let pick = rows_for("pressure.host_generics", "pick2_sum");
    assert_eq!(pick.len(), 2, "{pick:?}");
    let pair22 = pick
        .iter()
        .find(|row| row["canonical_args"] == "value:2|value:2")
        .expect("(2, 2) row");
    assert_eq!(pair22["args_spellings"], json!(["2", "2"]));
    let pair32 = pick
        .iter()
        .find(|row| row["canonical_args"] == "value:3|value:2")
        .expect("(3, 2) row");
    assert_eq!(pair32["args_spellings"], json!(["3", "2"]));
    assert_ne!(pair22["entry_function"], pair32["entry_function"]);
    let nominal = rows_for("pressure.host_generics", "id_value");
    assert_eq!(nominal.len(), 2, "{nominal:?}");
    assert_eq!(nominal[0]["args_spellings"], json!(["Point"]));
    // The same instantiation named by identity instead of by short
    // name shares the specialization entry while keeping its own
    // addressable row (spelling union, not spelling soup).
    assert_eq!(
        nominal[1]["args_spellings"],
        json!(["mncs:0.2:record-type:pressure.host_generics::Point::x%3Ai64%3By%3Ai64%3B"])
    );
    assert_eq!(
        nominal[1]["canonical_args"], nominal[0]["canonical_args"],
        "one instantiation, two addresses"
    );
    assert_eq!(
        nominal[1]["entry_function"], nominal[0]["entry_function"],
        "one instantiation, one entry"
    );
    assert!(
        nominal[0]["canonical_args"]
            .as_str()
            .is_some_and(|canonical| canonical.starts_with("type:record:mncs:")),
        "nominal canonical is identity-based: {nominal:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Frozen artifacts serve generic entrypoints with no program in sight:
/// compile (seeded by the corpus) plus `experiment execute` meets every
/// case on the artifact-only path, including the native backends.
#[test]
fn frozen_artifacts_serve_generic_entrypoints() {
    for backend in [
        "mncs-portable-wasm-mvp",
        "mncs-c11",
        "mncs-llvm-ir",
        "mncs-cranelift",
    ] {
        let dir = workspace(&format!("frozen-{}", backend.replace("mncs-", "")));
        let compiled = binary()
            .env("MNCS_LIBRARY_PATH", library_dir())
            .args([
                "compile",
                &source(),
                "--emit",
                "backend",
                "--target",
                backend,
            ])
            .arg("--corpus")
            .arg(corpus())
            .arg("--output-dir")
            .arg(&dir)
            .output()
            .expect("compile frozen artifact");
        assert!(
            compiled.status.success(),
            "{backend}: seeded compile exits 0: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = binary()
            .args(["experiment", "execute"])
            .arg(dir.join("backend.json"))
            .arg(corpus())
            .output()
            .expect("execute frozen artifact");
        assert!(
            executed.status.success(),
            "{backend}: frozen execute exits 0: {}",
            String::from_utf8_lossy(&executed.stderr)
        );
        let observations: Value =
            serde_json::from_slice(&executed.stdout).expect("observations JSON");
        let cases = observations.as_array().expect("observations array");
        assert_eq!(cases.len(), 14, "{backend}: every case runs once");
        for case in cases {
            assert_eq!(case["status"], "returned", "{backend}: {case:#}");
            assert_eq!(case["status_met"], true, "{backend}: {case:#}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// `mncs abi` advertises generic parameters so a host can spell
/// `type_arguments` without reading compiler internals: every generic
/// entry reports its parameters in order plus a machine-readable
/// requires flag, and every in-language instantiation is listed.
#[test]
fn abi_advertises_generic_parameters() {
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(["abi", &source()])
        .output()
        .expect("abi report");
    assert!(
        output.status.success(),
        "abi exits 0: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let abi: Value = serde_json::from_slice(&output.stdout).expect("abi JSON");
    let functions = abi.get("functions").expect("functions map");
    let local = &functions["grow_fill_local"];
    assert_eq!(local["requires_type_arguments"], true);
    assert_eq!(
        local["generic_params"],
        json!([{"name": "W", "kind": "nat"}]),
        "params in order: {local:#}"
    );
    let w8 = local["compiled_instantiations"]
        .as_array()
        .and_then(|insts| {
            insts
                .iter()
                .find(|inst| inst["canonical_args"] == "value:8")
        })
        .expect("in-language W=8 instantiation listed");
    assert!(
        w8["entry_function"]
            .as_str()
            .is_some_and(|entry| entry.starts_with("grow_fill_local__spec_")),
        "{w8:?}"
    );
    assert_eq!(
        functions["first_local"]["generic_params"],
        json!([{"name": "T", "kind": "type"}])
    );
    assert_eq!(
        functions["probe_same_fill"]["requires_type_arguments"],
        false
    );
    assert_eq!(
        functions["probe_same_fill"]["generic_params"],
        json!([]),
        "concrete entries stay bare"
    );
}

/// Targeting a generic function bare — no `type_arguments` — fails
/// closed on the direct reference executors with an explicit reason,
/// never by executing the unspecialized template.
#[test]
fn bare_generic_targets_fail_closed_with_an_explicit_reason() {
    for command in ["execute", "execute-ssa"] {
        let request = json!({
            "schema_version": "0.1",
            "target": {"module": "pressure.host_generics", "function": "grow_fill_local"},
            "arguments": [
                {"sequence": {"values": [{"integer": {"value": 0, "type": {"bits": 64, "signed": true}}}]}},
                {"integer": {"value": 1, "type": {"bits": 64, "signed": true}}}
            ],
            "step_budget": 8000
        });
        let mut child = binary()
            .env("MNCS_LIBRARY_PATH", library_dir())
            .args([command, &source(), "/dev/stdin"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("spawn direct executor");
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .expect("stdin")
            .write_all(request.to_string().as_bytes())
            .expect("write request");
        let output = child.wait_with_output().expect("direct result");
        assert_eq!(
            output.status.code(),
            Some(2),
            "{command}: invalid requests exit 2: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: Value = serde_json::from_slice(&output.stdout).expect("result JSON");
        assert_eq!(result["status"], "invalid_request", "{command}: {result:#}");
        assert!(
            result["failure"]["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("requires explicit type_arguments")),
            "{command}: explicit reason: {result:#}"
        );
    }
}

/// Malformed instantiations fail at elaboration with the same MNE codes
/// an in-language mistake reports — never as a silent skip, and never
/// as a miscompiled entry.
#[test]
fn malformed_seeds_fail_at_elaboration_with_mne_codes() {
    let dir = workspace("seeds");
    let integer =
        |value: i64| json!({"integer": {"value": value, "type": {"bits": 64, "signed": true}}});
    let sequence = |values: Vec<Value>| json!({"sequence": {"values": values}});
    let zeros = |count: usize| sequence(vec![integer(0); count]);
    let nat = |value: u64| json!({"kind": "nat", "value": value});
    let typ = |name: &str| json!({"kind": "type", "type": name});
    let module = "pressure.host_generics";
    // (name, target module, target function, arguments, type_arguments,
    //  expected diagnostic code).
    type SeedProbe<'a> = (&'a str, &'a str, &'a str, Vec<Value>, Vec<Value>, &'a str);
    let probes: Vec<SeedProbe<'_>> = vec![
        (
            "unknown-function",
            module,
            "nosuch",
            vec![zeros(8), integer(7)],
            vec![nat(8)],
            "MNE131",
        ),
        (
            "arity-mismatch",
            module,
            "grow_fill_local",
            vec![zeros(8), integer(7)],
            vec![nat(8), nat(8)],
            "MNE221",
        ),
        (
            "nat-for-type-parameter",
            module,
            "first_local",
            vec![integer(1)],
            vec![nat(8)],
            "MNE222",
        ),
        (
            "type-for-nat-parameter",
            module,
            "grow_fill_local",
            vec![zeros(8), integer(7)],
            vec![typ("i64")],
            "MNE222",
        ),
        (
            "value-spelling-for-type-parameter",
            module,
            "first_local",
            vec![integer(1)],
            vec![typ("8")],
            "MNE222",
        ),
        (
            "unknown-type-name",
            module,
            "first_local",
            vec![integer(1)],
            vec![typ("quat")],
            "MNE105",
        ),
        (
            "over-ceiling-nat",
            module,
            "grow_fill_local",
            vec![zeros(8), integer(7)],
            vec![nat(5000)],
            "MNE225",
        ),
        (
            "arguments-for-concrete-function",
            module,
            "probe_same_fill",
            vec![],
            vec![nat(8)],
            "MNE222",
        ),
    ];
    for (name, target_module, target_function, arguments, type_arguments, code) in probes {
        let corpus = json!({
            "schema_version": "0.1",
            "name": format!("seed-probe-{name}"),
            "cases": [{
                "id": "probe",
                "request": {
                    "schema_version": "0.1",
                    "target": {"module": target_module, "function": target_function},
                    "arguments": arguments,
                    "step_budget": 8000,
                    "type_arguments": type_arguments
                },
                "expected": [],
                "expected_status": "returned"
            }]
        });
        let path = dir.join(format!("seed-{name}.json"));
        std::fs::write(&path, corpus.to_string()).expect("write probe corpus");
        let output = binary()
            .env("MNCS_LIBRARY_PATH", library_dir())
            .args([
                "experiment",
                "run",
                &source(),
                "--backend",
                "mncs-research-bytecode",
            ])
            .arg("--corpus")
            .arg(&path)
            .output()
            .expect("run seed probe");
        assert!(
            !output.status.success(),
            "{name}: malformed seed must fail the experiment"
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains(code),
            "{name}: diagnostic names {code}: {stdout}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A `type_arguments` value outside the schema (here an unknown `kind`
/// tag) fails corpus loading itself: the boundary never parses a
/// half-shaped instantiation.
#[test]
fn malformed_type_argument_shapes_fail_corpus_loading() {
    let dir = workspace("shapes");
    let corpus = json!({
        "schema_version": "0.1",
        "name": "shape-probe",
        "cases": [{
            "id": "probe",
            "request": {
                "schema_version": "0.1",
                "target": {"module": "pressure.host_generics", "function": "grow_fill_local"},
                "arguments": [],
                "step_budget": 8000,
                "type_arguments": [{"kind": "bogus", "value": 8}]
            },
            "expected": [],
            "expected_status": "returned"
        }]
    });
    let path = dir.join("shape-probe.json");
    std::fs::write(&path, corpus.to_string()).expect("write probe corpus");
    let output = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "experiment",
            "run",
            &source(),
            "--backend",
            "mncs-research-bytecode",
        ])
        .arg("--corpus")
        .arg(&path)
        .output()
        .expect("run shape probe");
    assert!(
        !output.status.success(),
        "unknown type-argument shape must fail corpus loading"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown variant"),
        "failure names the bad shape: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Stripping the entrypoint map from an artifact is tampering, not a
/// downgrade: the identity gate refuses every case as stale-or-laundered
/// before entry resolution runs. (Genuine pre-0.4 artifacts — valid under
/// their own schema — take the explicit recompile diagnostic inside
/// `resolve_request_entry`, pinned by codegen unit tests, because the
/// identity gate already fails them closed first.)
#[test]
fn stripped_entrypoint_maps_fail_closed_at_the_identity_gate() {
    let dir = workspace("legacy");
    let compiled = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "compile",
            &source(),
            "--emit",
            "backend",
            "--target",
            "mncs-c11",
        ])
        .arg("--corpus")
        .arg(corpus())
        .arg("--output-dir")
        .arg(&dir)
        .output()
        .expect("compile seeded artifact");
    assert!(compiled.status.success());
    let mut artifact: Value = serde_json::from_slice(
        &std::fs::read(dir.join("backend.json")).expect("backend.json emitted"),
    )
    .expect("artifact JSON");
    artifact
        .as_object_mut()
        .expect("artifact object")
        .remove("generic_entrypoints");
    std::fs::write(dir.join("stripped.json"), artifact.to_string()).expect("stripped artifact");
    let executed = binary()
        .args(["experiment", "execute"])
        .arg(dir.join("stripped.json"))
        .arg(corpus())
        .output()
        .expect("execute stripped artifact");
    // Every refusal is InvalidRequest, so the run reports FAILURE while
    // still emitting the full observation array for inspection.
    assert!(
        !executed.status.success(),
        "tampered artifact must not report success"
    );
    let observations: Value = serde_json::from_slice(&executed.stdout).expect("observations JSON");
    let cases = observations.as_array().expect("observations array");
    assert_eq!(cases.len(), 14, "every case runs once");
    for case in cases {
        assert_eq!(case["status"], "invalid_request", "{case:#}");
        assert!(
            case["failure_reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("stale or laundered")),
            "identity gate names the tampering: {case:#}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Corpus lint resolves entries exactly like execution: the seeded
/// corpus lints clean, and a bare generic target reports the
/// requires-arguments refusal at authoring time.
#[test]
fn corpus_lint_resolves_generic_entries_like_execution() {
    let linted = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(["corpus", "lint", &source(), &corpus()])
        .output()
        .expect("lint seeded corpus");
    assert!(
        linted.status.success(),
        "seeded corpus lints clean: {}",
        String::from_utf8_lossy(&linted.stderr)
    );
    let dir = workspace("lint");
    let corpus = json!({
        "schema_version": "0.1",
        "name": "lint-probe",
        "cases": [{
            "id": "bare",
            "request": {
                "schema_version": "0.1",
                "target": {"module": "pressure.host_generics", "function": "vlen"},
                "arguments": [{"sequence": {"values": []}}],
                "step_budget": 8000
            },
            "expected": [],
            "expected_status": "invalid_request"
        }]
    });
    let path = dir.join("lint-probe.json");
    std::fs::write(&path, corpus.to_string()).expect("write probe corpus");
    let linted = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args(["corpus", "lint", &source()])
        .arg(&path)
        .output()
        .expect("lint bare corpus");
    assert!(
        !linted.status.success(),
        "bare generic target must not lint clean"
    );
    let stdout = String::from_utf8_lossy(&linted.stdout);
    assert!(
        stdout.contains("requires explicit type_arguments"),
        "lint names the refusal: {stdout}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Retained native sessions serve repeated and distinct instantiations
/// of one generic without cross-talk: the same frozen artifact answers
/// twice identically, and W=8 vs W=4 stay distinct entries.
#[test]
fn repeated_instantiation_calls_agree_without_cross_talk() {
    let dir = workspace("repeat");
    let compiled = binary()
        .env("MNCS_LIBRARY_PATH", library_dir())
        .args([
            "compile",
            &source(),
            "--emit",
            "backend",
            "--target",
            "mncs-c11",
        ])
        .arg("--corpus")
        .arg(corpus())
        .arg("--output-dir")
        .arg(&dir)
        .output()
        .expect("compile seeded artifact");
    assert!(compiled.status.success());
    let run = || {
        let executed = binary()
            .args(["experiment", "execute"])
            .arg(dir.join("backend.json"))
            .arg(corpus())
            .output()
            .expect("execute frozen artifact");
        assert!(executed.status.success());
        serde_json::from_slice::<Value>(&executed.stdout).expect("observations JSON")
    };
    let first = run();
    let second = run();
    assert_eq!(first, second, "repeated frozen runs agree exactly");
    let by_id = |observations: &Value, id: &str| {
        observations
            .as_array()
            .and_then(|cases| {
                cases
                    .iter()
                    .find(|case| case.get("case_id").and_then(Value::as_str) == Some(id))
            })
            .cloned()
            .unwrap_or(Value::Null)
    };
    assert_ne!(
        by_id(&first, "host-nat-fill")["returned"],
        by_id(&first, "host-nat-fill-4")["returned"],
        "distinct instantiations stay distinct"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
