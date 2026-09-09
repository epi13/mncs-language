//! MNCS-native JIT end-to-end evidence (`docs/jit-architecture.md`).
//!
//! Architecture under test: the JIT orchestration layer (session,
//! generations, bindings, invalidation, planning, profiling,
//! proof-aware publication, lifecycle) is implemented in MNCS
//! (`library/jit/`). Rust is only the thin provider driver: it
//! compiles definition sources through the existing bootstrap
//! pipeline, invokes compiled artifacts, supplies wall-clock
//! observations and content fingerprints, and threads the MNCS-owned
//! session value between calls. No orchestration decision lives in
//! this file; every generation/binding/invalidation outcome is
//! computed by MNCS code and only asserted here.
//!
//! Provider split (explicit, no overclaim):
//! - orchestration threading uses `mncs-research-bytecode` (fast,
//!   deterministic) EXCEPT `jit_cranelift_threads_session_values`,
//!   which replays a core flow on `mncs-cranelift` to prove session
//!   values round-trip through native calls;
//! - every definition compiles and invokes through `mncs-cranelift`
//!   (the first native provider); corpora prove the same MNCS logic
//!   executes identically on all backends
//!   (`crates/mncs-cli/tests/jit_orchestration.rs`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

use mncs_compiler::{ModuleResolver, ReferenceCompiler};
use mncs_embed::{Artifact, CallOptions, Session};
use mncs_model::ArtifactRepresentation;
use mncs_syntax::{SourceArtifactKind, SourceEnvelope, SourceOrigin, SourceOriginKind};
use serde_json::{json, Value};

// ---------------------------------------------------------------------------
// Library resolution (test-side only; mirrors the CLI FileModuleResolver
// candidate for `library/jit/<name>.mncs`).
// ---------------------------------------------------------------------------

fn library_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../library")
}

struct JitLibraryResolver {
    root: PathBuf,
}

impl JitLibraryResolver {
    fn relative_target(module: &str) -> Option<&'static str> {
        match module {
            "mncs.jit.types.v1" => Some("jit/types.mncs"),
            "mncs.jit.depends.v1" => Some("jit/depends.mncs"),
            "mncs.jit.lifecycle.v1" => Some("jit/lifecycle.mncs"),
            "mncs.jit.plan.v1" => Some("jit/plan.mncs"),
            "mncs.jit.profile.v1" => Some("jit/profile.mncs"),
            "mncs.jit.proof.v1" => Some("jit/proof.mncs"),
            "mncs.jit.session.v1" => Some("jit/session.mncs"),
            "mncs.jit.binding.v1" => Some("jit/binding.mncs"),
            "mncs.core.logic.v1" => Some("core/logic.mncs"),
            _ => None,
        }
    }
}

impl ModuleResolver for JitLibraryResolver {
    fn resolve(&self, module: &str) -> Option<SourceEnvelope> {
        let relative = Self::relative_target(module)?;
        let text = std::fs::read_to_string(self.root.join(relative)).ok()?;
        Some(SourceEnvelope::new(
            SourceArtifactKind::Program,
            format!("jit-e2e:{module}"),
            SourceOrigin {
                kind: SourceOriginKind::Path,
                locator: Some(relative.to_owned()),
            },
            text,
        ))
    }
}

// ---------------------------------------------------------------------------
// Compilation helpers (bootstrap provider driver).
// ---------------------------------------------------------------------------

/// Compile the MNCS JIT orchestration program (rooted at
/// `library/jit/binding.mncs`, which transitively links session,
/// depends, lifecycle, proof, types, and logic) to a backend artifact.
fn compile_orchestration(backend: &str) -> (Artifact, u128) {
    let root = library_root();
    let text = std::fs::read_to_string(root.join("jit/binding.mncs")).expect("read binding.mncs");
    let compiler = ReferenceCompiler::default();
    let envelope = SourceEnvelope::inline(SourceArtifactKind::Program, "jit-e2e-root", text);
    let resolver = JitLibraryResolver { root };
    let started = Instant::now();
    let front_end = compiler.front_end_with_resolver(envelope, &resolver);
    assert!(
        front_end.is_valid(),
        "JIT orchestration front end must be valid: {:#?}",
        front_end.diagnostics
    );
    let program = front_end.program.expect("elaborated JIT program");
    let emit: std::collections::BTreeSet<ArtifactRepresentation> = [
        ArtifactRepresentation::Semantic,
        ArtifactRepresentation::Hir,
        ArtifactRepresentation::Ssa,
        ArtifactRepresentation::TargetLoweringPlan,
        ArtifactRepresentation::BackendArtifact,
    ]
    .into_iter()
    .collect();
    let request = compiler
        .request_for_program_with_backend(&program, emit, backend)
        .expect("backend request");
    let compilation = compiler.compile(request, &program);
    assert!(
        matches!(
            compilation.status,
            mncs_model::CompilationStatus::Completed
                | mncs_model::CompilationStatus::CompletedWithUnresolvedObligations
        ),
        "JIT orchestration must compile for {backend}: {:?}",
        compilation.status
    );
    let artifact = compilation.emissions.backend.expect("backend artifact");
    let elapsed_ms = started.elapsed().as_millis();
    let bytes = serde_json::to_vec(&artifact).expect("artifact serializes");
    let artifact = Artifact::from_json(&bytes).expect("artifact identity validates");
    (artifact, elapsed_ms)
}

/// Compile one definition source to a native provider artifact.
/// Returns `None` when the source is rejected (invalid submission).
fn compile_definition(source: &str) -> Option<(Artifact, u128)> {
    let started = Instant::now();
    let artifact = match Artifact::from_source(source, "mncs-cranelift") {
        Ok(artifact) => artifact,
        Err(_) => return None,
    };
    Some((artifact, started.elapsed().as_millis()))
}

/// Test-only content fingerprint (FNV-1a/64 over the source bytes).
/// Production would truncate a sha256; the boundary contract is only
/// that the host supplies a stable u64 per distinct source text.
fn content_hash(source: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in source.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

// ---------------------------------------------------------------------------
// JSON value helpers (canonical ExecutionValue documents).
// ---------------------------------------------------------------------------

fn iu(value: u64) -> Value {
    json!({"integer": {"value": value, "type": {"bits": 64, "signed": false}}})
}

fn seq_u64(values: &[u64]) -> Value {
    json!({"sequence": {"values": values.iter().map(|v| iu(*v)).collect::<Vec<_>>()}})
}

/// Navigate a returned record value: `field` of a `{"record": ...}`.
fn rf<'v>(value: &'v Value, field: &str) -> &'v Value {
    &value["record"]["fields"]
        .as_array()
        .unwrap_or_else(|| panic!("record fields missing in {value}"))
        .iter()
        .find(|entry| entry[0] == field)
        .unwrap_or_else(|| panic!("field {field} missing in {value}"))[1]
}

fn ru(value: &Value) -> u64 {
    value["integer"]["value"]
        .as_u64()
        .unwrap_or_else(|| panic!("u64 integer missing in {value}"))
}

fn rb(value: &Value) -> bool {
    value["boolean"]["value"]
        .as_bool()
        .unwrap_or_else(|| panic!("boolean missing in {value}"))
}

fn call_json(session: &Session, module: &str, function: &str, args: Vec<Value>) -> Vec<Value> {
    let args_json = serde_json::to_string(&args).expect("args serialize");
    let output = session
        .call_json(
            module,
            function,
            &args_json,
            &CallOptions::budgeted(2_000_000),
        )
        .unwrap_or_else(|error| panic!("{module}::{function} call failed: {error}"));
    assert_eq!(
        output.status, "returned",
        "{module}::{function} status: {} ({:?})",
        output.status, output.failure_reason
    );
    output
        .returned
        .iter()
        .map(|value| serde_json::to_value(value).expect("returned serializes"))
        .collect()
}

fn call_no_args(session: &Session, module: &str, function: &str) -> Vec<Value> {
    call_json(session, module, function, vec![])
}

// ---------------------------------------------------------------------------
// Definition sources (profile 0.6 probe shape, Cranelift-realized).
// ---------------------------------------------------------------------------

const FOO_V1: &str = "mncs 0.6;\nmodule probe.jitdef;\nfn foo(x: i32) -> (result: i32) {\n    if x >= 10 {\n        return x +% 40;\n    }\n    return x +% 1;\n}\n";

const FOO_V2: &str = "mncs 0.6;\nmodule probe.jitdef;\nfn foo(x: i32) -> (result: i32) {\n    if x >= 10 {\n        return x +% 400;\n    }\n    return x +% 2;\n}\n";

const BAR_V1: &str = "mncs 0.6;\nmodule probe.jitdef;\nfn foo(x: i32) -> (result: i32) {\n    if x >= 10 {\n        return x +% 40;\n    }\n    return x +% 1;\n}\nfn bar(x: i32) -> (result: i32) {\n    return foo(x) +% 100;\n}\n";

const INVALID_DEF: &str = "mncs 0.6;\nmodule probe.jitdef;\nfn foo(x: i32) -> (result: i32) {\n    return nosuchfn(x);\n}\n";

fn invoke_i32(session: &Session, function: &str, input: i32) -> i128 {
    let args = serde_json::to_string(&vec![
        json!({"integer": {"value": input, "type": {"bits": 32, "signed": true}}}),
    ])
    .expect("arg serializes");
    let output = session
        .call_json(
            "probe.jitdef",
            function,
            &args,
            &CallOptions::budgeted(8192),
        )
        .expect("definition call succeeds");
    assert_eq!(
        output.status, "returned",
        "definition {function} must return"
    );
    match &output.returned[..] {
        [mncs_model::ExecutionValue::Integer { value, .. }] => *value,
        other => panic!("integer verdict expected, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Chapter 1: basic execution + persistent session + safe redefinition.
// ---------------------------------------------------------------------------

/// End-to-end: MNCS definition -> MNCS JIT session -> generation ->
/// Cranelift provider -> native invoke -> logical binding, then safe
/// redefinition with generational publication and rollback of a
/// failed generation.
#[test]
fn jit_end_to_end_cranelift_definitions_with_mncs_orchestration() {
    let mut timings: Vec<(&str, u128)> = Vec::new();

    let started = Instant::now();
    let (orchestration, compile_ms) = compile_orchestration("mncs-research-bytecode");
    timings.push(("orchestration_bytecode_compile", compile_ms));
    let jit = Session::open(orchestration).expect("open orchestration session");
    timings.push(("orchestration_session_open", started.elapsed().as_millis()));

    // 1. Create session; generations start at era 1.
    let state = call_no_args(&jit, "mncs.jit.session.v1", "create")[0].clone();
    assert_eq!(ru(rf(&state, "semantic_generation")), 1);
    assert_eq!(ru(rf(&state, "proof_generation")), 1);
    assert_eq!(ru(rf(&state, "def_count")), 0);

    // 2. Define foo (logical allocation) and bar (-> foo edge).
    let no_deps = seq_u64(&[0, 0, 0, 0]);
    let d_foo = call_json(
        &jit,
        "mncs.jit.session.v1",
        "define",
        vec![
            state.clone(),
            iu(0),
            iu(content_hash(FOO_V1)),
            iu(0),
            iu(0),
            no_deps.clone(),
            iu(0),
            iu(0),
        ],
    )[0]
    .clone();
    assert!(rb(rf(&d_foo, "accepted")));
    let foo_logical = ru(rf(&d_foo, "logical"));
    assert_eq!(foo_logical, 1);
    assert_eq!(ru(rf(&d_foo, "generation")), 1);
    let mut state = rf(&d_foo, "session").clone();

    let bar_deps = seq_u64(&[foo_logical, 0, 0, 0]);
    let d_bar = call_json(
        &jit,
        "mncs.jit.session.v1",
        "define",
        vec![
            state.clone(),
            iu(0),
            iu(content_hash(BAR_V1)),
            iu(0),
            iu(0),
            bar_deps,
            iu(1),
            iu(0),
        ],
    )[0]
    .clone();
    let bar_logical = ru(rf(&d_bar, "logical"));
    assert_eq!(bar_logical, 2);
    state = rf(&d_bar, "session").clone();

    // 3. Compile bar's program (foo v1 + bar) through Cranelift and publish both.
    let started = Instant::now();
    let (bar_artifact, bar_compile_ms) = compile_definition(BAR_V1).expect("bar program compiles");
    timings.push(("bar_cranelift_compile", bar_compile_ms));
    let bar_provider = Session::open(bar_artifact).expect("open bar provider session");
    timings.push(("bar_provider_open", started.elapsed().as_millis()));
    assert_eq!(invoke_i32(&bar_provider, "foo", 20), 60);
    assert_eq!(invoke_i32(&bar_provider, "bar", 20), 160);

    let n_foo = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "note_compiled",
        vec![
            state.clone(),
            iu(foo_logical),
            iu(1),
            iu(1),
            iu(bar_compile_ms as u64),
            iu(1),
            iu(1),
            iu(0),
        ],
    )[0]
    .clone();
    assert!(rb(rf(&n_foo, "accepted")));
    let foo_code_v1 = ru(rf(&n_foo, "artifact_code"));
    state = rf(&n_foo, "session").clone();

    let n_bar = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "note_compiled",
        vec![
            state.clone(),
            iu(bar_logical),
            iu(2),
            iu(1),
            iu(bar_compile_ms as u64),
            iu(1),
            iu(1),
            iu(0),
        ],
    )[0]
    .clone();
    let bar_code_v1 = ru(rf(&n_bar, "artifact_code"));
    assert!(bar_code_v1 != foo_code_v1, "distinct executable identities");
    state = rf(&n_bar, "session").clone();

    // Definitions keep their own provider sessions keyed by MNCS artifact code.
    let mut providers: HashMap<u64, Session> = HashMap::new();

    let p_foo = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "publish",
        vec![state.clone(), iu(foo_logical), iu(foo_code_v1)],
    )[0]
    .clone();
    assert!(rb(rf(&p_foo, "accepted")));
    state = rf(&p_foo, "session").clone();
    let p_bar = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "publish",
        vec![state.clone(), iu(bar_logical), iu(bar_code_v1)],
    )[0]
    .clone();
    assert!(rb(rf(&p_bar, "accepted")));
    state = rf(&p_bar, "session").clone();
    providers.insert(foo_code_v1, bar_provider);

    // 4. Resolve + invoke through the logical binding.
    let r = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "resolve",
        vec![state.clone(), iu(foo_logical)],
    )[0]
    .clone();
    assert!(rb(rf(&r, "found")));
    assert_eq!(ru(rf(&r, "generation")), 1);
    assert_eq!(ru(rf(&r, "artifact")), foo_code_v1);
    assert_eq!(ru(rf(&r, "state_code")), 0);
    assert!(rb(rf(&r, "proof_current")));
    assert_eq!(invoke_i32(&providers[&foo_code_v1], "foo", 20), 60);

    // 5. Redefine foo (body-only): binding goes Stale, old code still alive.
    let d_foo2 = call_json(
        &jit,
        "mncs.jit.session.v1",
        "define",
        vec![
            state.clone(),
            iu(foo_logical),
            iu(content_hash(FOO_V2)),
            iu(0),
            iu(0),
            no_deps.clone(),
            iu(0),
            iu(0),
        ],
    )[0]
    .clone();
    assert_eq!(ru(rf(&d_foo2, "reason")), 1);
    assert_eq!(ru(rf(&d_foo2, "generation")), 3);
    state = rf(&d_foo2, "session").clone();
    let r_stale = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "resolve",
        vec![state.clone(), iu(foo_logical)],
    )[0]
    .clone();
    assert_eq!(ru(rf(&r_stale, "state_code")), 1);
    assert_eq!(invoke_i32(&providers[&foo_code_v1], "foo", 5), 6);

    // 6. Compile + publish generation 2; the binding swings atomically.
    let (foo2_artifact, foo2_ms) = compile_definition(FOO_V2).expect("foo v2 compiles");
    timings.push(("foo_v2_cranelift_compile", foo2_ms));
    let foo2_provider = Session::open(foo2_artifact).expect("open foo v2 provider");
    let n_foo2 = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "note_compiled",
        vec![
            state.clone(),
            iu(foo_logical),
            iu(3),
            iu(1),
            iu(foo2_ms as u64),
            iu(1),
            iu(1),
            iu(0),
        ],
    )[0]
    .clone();
    let foo_code_v2 = ru(rf(&n_foo2, "artifact_code"));
    assert!(foo_code_v2 != foo_code_v1);
    state = rf(&n_foo2, "session").clone();
    let p_foo2 = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "publish",
        vec![state.clone(), iu(foo_logical), iu(foo_code_v2)],
    )[0]
    .clone();
    assert!(rb(rf(&p_foo2, "accepted")));
    state = rf(&p_foo2, "session").clone();
    providers.insert(foo_code_v2, foo2_provider);
    let r2 = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "resolve",
        vec![state.clone(), iu(foo_logical)],
    )[0]
    .clone();
    assert_eq!(ru(rf(&r2, "generation")), 3);
    assert_eq!(ru(rf(&r2, "artifact")), foo_code_v2);
    assert_eq!(invoke_i32(&providers[&foo_code_v2], "foo", 5), 7);
    // Superseded generation stays alive and directly invocable.
    assert_eq!(invoke_i32(&providers[&foo_code_v1], "foo", 5), 6);
    let old_art = call_json(
        &jit,
        "mncs.jit.session.v1",
        "artifact_of",
        vec![state.clone(), iu(foo_code_v1)],
    )[0]
    .clone();
    assert_eq!(ru(rf(&old_art, "lifecycle_code")), 2);

    // 7. Failed generation: define v3, compilation fails, abandon restores v2.
    let d_foo3 = call_json(
        &jit,
        "mncs.jit.session.v1",
        "define",
        vec![
            state.clone(),
            iu(foo_logical),
            iu(content_hash(INVALID_DEF)),
            iu(0),
            iu(0),
            no_deps.clone(),
            iu(0),
            iu(0),
        ],
    )[0]
    .clone();
    assert_eq!(ru(rf(&d_foo3, "generation")), 4);
    state = rf(&d_foo3, "session").clone();
    assert!(
        compile_definition(INVALID_DEF).is_none(),
        "invalid source must not compile"
    );
    let back = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "abandon",
        vec![state.clone(), iu(foo_logical)],
    )[0]
    .clone();
    assert!(rb(rf(&back, "restored")));
    state = rf(&back, "session").clone();
    let r3 = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "resolve",
        vec![state.clone(), iu(foo_logical)],
    )[0]
    .clone();
    assert_eq!(ru(rf(&r3, "generation")), 3);
    assert_eq!(ru(rf(&r3, "artifact")), foo_code_v2);
    assert_eq!(ru(rf(&r3, "state_code")), 0);
    assert_eq!(invoke_i32(&providers[&foo_code_v2], "foo", 5), 7);

    eprintln!("jit e2e timings (ms): {timings:?}");
}

// ---------------------------------------------------------------------------
// Chapter 2: dependency policy (pure orchestration, no native compiles).
// ---------------------------------------------------------------------------

/// `foo depends on bar`: a Signature change stales foo
/// (RequiresRecompile); a BodyOnly change leaves it Valid.
#[test]
fn jit_dependency_policy_signature_vs_body() {
    let (orchestration, _) = compile_orchestration("mncs-research-bytecode");
    let jit = Session::open(orchestration).expect("open orchestration session");
    let no_deps = seq_u64(&[0, 0, 0, 0]);

    let state = call_no_args(&jit, "mncs.jit.session.v1", "create")[0].clone();
    let d_bar = call_json(
        &jit,
        "mncs.jit.session.v1",
        "define",
        vec![
            state,
            iu(0),
            iu(11),
            iu(0),
            iu(0),
            no_deps.clone(),
            iu(0),
            iu(0),
        ],
    )[0]
    .clone();
    let mut state = rf(&d_bar, "session").clone();
    let bar = ru(rf(&d_bar, "logical"));
    let d_foo = call_json(
        &jit,
        "mncs.jit.session.v1",
        "define",
        vec![
            state,
            iu(0),
            iu(22),
            iu(0),
            iu(0),
            seq_u64(&[bar, 0, 0, 0]),
            iu(1),
            iu(0),
        ],
    )[0]
    .clone();
    let foo = ru(rf(&d_foo, "logical"));
    state = rf(&d_foo, "session").clone();

    // Publish both so foo's binding is Valid before the change.
    for (logical, generation) in [(bar, 1), (foo, 2)] {
        let n = call_json(
            &jit,
            "mncs.jit.binding.v1",
            "note_compiled",
            vec![
                state.clone(),
                iu(logical),
                iu(generation),
                iu(1),
                iu(3),
                iu(1),
                iu(1),
                iu(0),
            ],
        )[0]
        .clone();
        state = rf(&n, "session").clone();
    }
    // Artifact codes are sequential from 1.
    for (logical, code) in [(bar, 1), (foo, 2)] {
        let p = call_json(
            &jit,
            "mncs.jit.binding.v1",
            "publish",
            vec![state.clone(), iu(logical), iu(code)],
        )[0]
        .clone();
        assert!(rb(rf(&p, "accepted")));
        state = rf(&p, "session").clone();
    }

    // Signature change to bar: foo must require recompile.
    let d_bar2 = call_json(
        &jit,
        "mncs.jit.session.v1",
        "define",
        vec![
            state.clone(),
            iu(bar),
            iu(33),
            iu(1),
            iu(0),
            no_deps.clone(),
            iu(0),
            iu(0),
        ],
    )[0]
    .clone();
    state = rf(&d_bar2, "session").clone();
    let q_foo = call_json(
        &jit,
        "mncs.jit.session.v1",
        "binding_of",
        vec![state.clone(), iu(foo)],
    )[0]
    .clone();
    assert!(rb(rf(&q_foo, "found")));
    assert_eq!(ru(rf(&q_foo, "state_code")), 1, "foo must be Stale");
    assert_eq!(ru(rf(&q_foo, "stale_reason")), 2, "reason dep-signature");

    // Rebuild bar then foo (new source bytes => new generation), re-publish.
    let n_bar = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "note_compiled",
        vec![
            state.clone(),
            iu(bar),
            iu(3),
            iu(1),
            iu(4),
            iu(1),
            iu(1),
            iu(0),
        ],
    )[0]
    .clone();
    state = rf(&n_bar, "session").clone();
    let p_bar = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "publish",
        vec![state.clone(), iu(bar), iu(3)],
    )[0]
    .clone();
    assert!(rb(rf(&p_bar, "accepted")));
    state = rf(&p_bar, "session").clone();
    let d_foo2 = call_json(
        &jit,
        "mncs.jit.session.v1",
        "define",
        vec![
            state.clone(),
            iu(foo),
            iu(44),
            iu(0),
            iu(0),
            seq_u64(&[bar, 0, 0, 0]),
            iu(1),
            iu(0),
        ],
    )[0]
    .clone();
    state = rf(&d_foo2, "session").clone();
    let n_foo = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "note_compiled",
        vec![
            state.clone(),
            iu(foo),
            iu(4),
            iu(1),
            iu(4),
            iu(1),
            iu(1),
            iu(0),
        ],
    )[0]
    .clone();
    state = rf(&n_foo, "session").clone();
    let p_foo = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "publish",
        vec![state.clone(), iu(foo), iu(4)],
    )[0]
    .clone();
    assert!(rb(rf(&p_foo, "accepted")));
    state = rf(&p_foo, "session").clone();
    let q_foo2 = call_json(
        &jit,
        "mncs.jit.session.v1",
        "binding_of",
        vec![state.clone(), iu(foo)],
    )[0]
    .clone();
    assert_eq!(ru(rf(&q_foo2, "state_code")), 0, "rebuilt foo is Valid");

    // BodyOnly change to bar: foo stays Valid and keeps routing.
    let d_bar3 = call_json(
        &jit,
        "mncs.jit.session.v1",
        "define",
        vec![
            state.clone(),
            iu(bar),
            iu(55),
            iu(0),
            iu(0),
            no_deps.clone(),
            iu(0),
            iu(0),
        ],
    )[0]
    .clone();
    state = rf(&d_bar3, "session").clone();
    let q_foo3 = call_json(
        &jit,
        "mncs.jit.session.v1",
        "binding_of",
        vec![state.clone(), iu(foo)],
    )[0]
    .clone();
    assert_eq!(
        ru(rf(&q_foo3, "state_code")),
        0,
        "body-only change keeps foo Valid"
    );
    assert_eq!(
        ru(rf(&q_foo3, "artifact")),
        4,
        "foo still routes to its artifact"
    );
}

// ---------------------------------------------------------------------------
// Chapter 3: proof era + lifecycle + close (pure orchestration).
// ---------------------------------------------------------------------------

/// Proof-era advance blocks stale publication; retirement freezes
/// bindings; close freezes the session deterministically.
#[test]
fn jit_proof_era_and_lifecycle() {
    let (orchestration, _) = compile_orchestration("mncs-research-bytecode");
    let jit = Session::open(orchestration).expect("open orchestration session");
    let no_deps = seq_u64(&[0, 0, 0, 0]);

    let state = call_no_args(&jit, "mncs.jit.session.v1", "create")[0].clone();
    let d = call_json(
        &jit,
        "mncs.jit.session.v1",
        "define",
        vec![
            state,
            iu(0),
            iu(101),
            iu(0),
            iu(7),
            no_deps.clone(),
            iu(0),
            iu(0),
        ],
    )[0]
    .clone();
    let mut state = rf(&d, "session").clone();
    let logical = ru(rf(&d, "logical"));

    // Generation 1 publishes under era (1, 1).
    let n1 = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "note_compiled",
        vec![
            state.clone(),
            iu(logical),
            iu(1),
            iu(1),
            iu(9),
            iu(1),
            iu(1),
            iu(7),
        ],
    )[0]
    .clone();
    state = rf(&n1, "session").clone();
    let p1 = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "publish",
        vec![state.clone(), iu(logical), iu(1)],
    )[0]
    .clone();
    assert!(rb(rf(&p1, "accepted")));
    state = rf(&p1, "session").clone();

    // Era advance: the published generation 1 binding reports proof-stale.
    let aged = call_json(
        &jit,
        "mncs.jit.session.v1",
        "advance_proof",
        vec![state.clone()],
    )[0]
    .clone();
    state = aged;
    let r_old = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "resolve",
        vec![state.clone(), iu(logical)],
    )[0]
    .clone();
    assert_eq!(ru(rf(&r_old, "generation")), 1);
    assert_eq!(ru(rf(&r_old, "state_code")), 0);
    assert!(
        !rb(rf(&r_old, "proof_current")),
        "old binding reports proof-stale"
    );
    let d2 = call_json(
        &jit,
        "mncs.jit.session.v1",
        "define",
        vec![
            state.clone(),
            iu(logical),
            iu(202),
            iu(0),
            iu(7),
            no_deps.clone(),
            iu(0),
            iu(0),
        ],
    )[0]
    .clone();
    state = rf(&d2, "session").clone();
    // Introspection stays tied to the generation it was validated under.
    let dp = call_json(
        &jit,
        "mncs.jit.session.v1",
        "definition_proof",
        vec![state.clone(), iu(logical), iu(2)],
    )[0]
    .clone();
    assert!(rb(rf(&dp, "found")));
    assert_eq!(ru(rf(&dp, "semantic_generation")), 1);
    assert_eq!(ru(rf(&dp, "proof_generation")), 2);
    assert_eq!(ru(rf(&dp, "assumption_bits")), 7);
    let dp_missing = call_json(
        &jit,
        "mncs.jit.session.v1",
        "definition_proof",
        vec![state.clone(), iu(logical), iu(99)],
    )[0]
    .clone();
    assert!(!rb(rf(&dp_missing, "found")));
    let n2 = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "note_compiled",
        vec![
            state.clone(),
            iu(logical),
            iu(2),
            iu(1),
            iu(9),
            iu(1),
            iu(1),
            iu(7),
        ],
    )[0]
    .clone();
    state = rf(&n2, "session").clone();
    let p2 = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "publish",
        vec![state.clone(), iu(logical), iu(2)],
    )[0]
    .clone();
    assert!(
        !rb(rf(&p2, "accepted")),
        "stale-proof publish must be refused"
    );
    assert_eq!(ru(rf(&p2, "reason")), 5);
    // The refused publish leaves generation 2 Stale with no artifact;
    // its own definition era is current (only the artifact was stale).
    let r = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "resolve",
        vec![state.clone(), iu(logical)],
    )[0]
    .clone();
    assert_eq!(ru(rf(&r, "generation")), 2);
    assert_eq!(ru(rf(&r, "state_code")), 1);
    assert!(rb(rf(&r, "proof_current")));

    // Same generation compiled under the NEW era publishes.
    let n3 = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "note_compiled",
        vec![
            state.clone(),
            iu(logical),
            iu(2),
            iu(1),
            iu(9),
            iu(1),
            iu(2),
            iu(7),
        ],
    )[0]
    .clone();
    state = rf(&n3, "session").clone();
    let p3 = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "publish",
        vec![state.clone(), iu(logical), iu(3)],
    )[0]
    .clone();
    assert!(rb(rf(&p3, "accepted")));
    state = rf(&p3, "session").clone();

    // Retire the active artifact: binding goes Stale, republication refused.
    let k = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "retire_artifact",
        vec![state.clone(), iu(3)],
    )[0]
    .clone();
    assert!(rb(rf(&k, "retired")));
    state = rf(&k, "session").clone();
    let r_after = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "resolve",
        vec![state.clone(), iu(logical)],
    )[0]
    .clone();
    assert_eq!(ru(rf(&r_after, "state_code")), 1);
    let repub = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "publish",
        vec![state.clone(), iu(logical), iu(3)],
    )[0]
    .clone();
    assert!(
        !rb(rf(&repub, "accepted")),
        "retired artifacts never reactivate"
    );

    // Close: deterministic shutdown; further mutation refused; idempotent.
    let shut = call_json(&jit, "mncs.jit.session.v1", "close", vec![state.clone()])[0].clone();
    assert!(rb(rf(&shut, "closed")));
    let shut2 = call_json(&jit, "mncs.jit.session.v1", "close", vec![shut.clone()])[0].clone();
    assert!(rb(rf(&shut2, "closed")));
    let after_close = call_json(
        &jit,
        "mncs.jit.session.v1",
        "define",
        vec![
            shut,
            iu(0),
            iu(303),
            iu(0),
            iu(0),
            no_deps.clone(),
            iu(0),
            iu(0),
        ],
    )[0]
    .clone();
    assert!(!rb(rf(&after_close, "accepted")));
    assert_eq!(ru(rf(&after_close, "reason")), 2);
}

// ---------------------------------------------------------------------------
// Chapter 4: the same core flow threaded through Cranelift machine code.
// ---------------------------------------------------------------------------

/// Session values round-trip through Cranelift-compiled orchestration
/// calls: create/define/note/publish/resolve execute natively.
#[test]
fn jit_cranelift_threads_session_values() {
    let (orchestration, compile_ms) = compile_orchestration("mncs-cranelift");
    eprintln!("jit cranelift orchestration compile: {compile_ms} ms");
    let started = Instant::now();
    let jit = Session::open(orchestration).expect("open cranelift orchestration");
    eprintln!(
        "jit cranelift orchestration open: {} ms",
        started.elapsed().as_millis()
    );
    let no_deps = seq_u64(&[0, 0, 0, 0]);

    let state = call_no_args(&jit, "mncs.jit.session.v1", "create")[0].clone();
    let d = call_json(
        &jit,
        "mncs.jit.session.v1",
        "define",
        vec![state, iu(0), iu(1001), iu(0), iu(0), no_deps, iu(0), iu(0)],
    )[0]
    .clone();
    assert!(rb(rf(&d, "accepted")));
    assert_eq!(ru(rf(&d, "logical")), 1);
    let mut state = rf(&d, "session").clone();

    let n = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "note_compiled",
        vec![
            state.clone(),
            iu(1),
            iu(1),
            iu(1),
            iu(11),
            iu(1),
            iu(1),
            iu(0),
        ],
    )[0]
    .clone();
    assert!(rb(rf(&n, "accepted")));
    assert_eq!(ru(rf(&n, "artifact_code")), 1);
    state = rf(&n, "session").clone();

    let p = call_json(
        &jit,
        "mncs.jit.binding.v1",
        "publish",
        vec![state.clone(), iu(1), iu(1)],
    )[0]
    .clone();
    assert!(rb(rf(&p, "accepted")));
    state = rf(&p, "session").clone();

    let r = call_json(&jit, "mncs.jit.binding.v1", "resolve", vec![state, iu(1)])[0].clone();
    assert!(rb(rf(&r, "found")));
    assert_eq!(ru(rf(&r, "generation")), 1);
    assert_eq!(ru(rf(&r, "artifact")), 1);
    assert_eq!(ru(rf(&r, "state_code")), 0);
    assert!(rb(rf(&r, "proof_current")));
}

// ---------------------------------------------------------------------------
// Chapter 5: determinism.
// ---------------------------------------------------------------------------

/// Two fresh sessions replaying the same committed op sequence reach
/// identical logical bindings.
#[test]
fn jit_deterministic_bindings_for_committed_state() {
    let (orchestration, _) = compile_orchestration("mncs-research-bytecode");
    let jit = Session::open(orchestration).expect("open orchestration session");
    let no_deps = seq_u64(&[0, 0, 0, 0]);

    fn replay(jit: &Session, no_deps: &Value) -> (u64, u64, u64) {
        let state = call_no_args(jit, "mncs.jit.session.v1", "create")[0].clone();
        let d = call_json(
            jit,
            "mncs.jit.session.v1",
            "define",
            vec![
                state,
                iu(0),
                iu(777),
                iu(1),
                iu(3),
                no_deps.clone(),
                iu(0),
                iu(0),
            ],
        )[0]
        .clone();
        let mut state = rf(&d, "session").clone();
        let logical = ru(rf(&d, "logical"));
        let n = call_json(
            jit,
            "mncs.jit.binding.v1",
            "note_compiled",
            vec![
                state.clone(),
                iu(logical),
                iu(1),
                iu(1),
                iu(6),
                iu(1),
                iu(1),
                iu(3),
            ],
        )[0]
        .clone();
        let code = ru(rf(&n, "artifact_code"));
        state = rf(&n, "session").clone();
        let p = call_json(
            jit,
            "mncs.jit.binding.v1",
            "publish",
            vec![state.clone(), iu(logical), iu(code)],
        )[0]
        .clone();
        assert!(rb(rf(&p, "accepted")));
        state = rf(&p, "session").clone();
        let r = call_json(
            jit,
            "mncs.jit.binding.v1",
            "resolve",
            vec![state, iu(logical)],
        )[0]
        .clone();
        (
            ru(rf(&r, "generation")),
            ru(rf(&r, "artifact")),
            ru(rf(&r, "state_code")),
        )
    }

    assert_eq!(replay(&jit, &no_deps), replay(&jit, &no_deps));
    assert_eq!(replay(&jit, &no_deps), (1, 1, 0));
}
