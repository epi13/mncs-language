use mncs_compiler::{ModuleResolver, ReferenceCompiler};
use mncs_embed::{Artifact, CallOptions, CallableReference, Grant, Session};
use mncs_model::{
    ArtifactRepresentation, ExecutionTypeArgument, ExecutionValue, HostExecutionValue,
    HostGenericSeedRequest, IntegerType, SemanticId,
};
use mncs_syntax::{SourceArtifactKind, SourceEnvelope};
use std::collections::BTreeSet;
use std::ffi::{CStr, CString};

const MODULE: &str = "probe.identity";
const OTHER_MODULE: &str = "probe.identity_extra";
const SOURCE: &str = "mncs 0.18;\nmodule probe.identity;\nfn plus_one(value: i64) -> (result: i64) { return value + 1; }\nfn times_two(value: i64) -> (result: i64) { return value * 2; }\nfn echo<T>(value: T) -> (result: T) { return value; }\nfn read_clock() -> (result: u64) capability ticker effect clock_read authorized_by ticker { return clock_read(); }\n";
const MULTI_MODULE_SOURCE: &str = include_str!("../../../tests/probe/identity.mncs");
const OTHER_SOURCE: &str = include_str!("../../../tests/probe/identity_extra.mncs");

struct IdentityModuleResolver;

impl ModuleResolver for IdentityModuleResolver {
    fn resolve(&self, module: &str) -> Option<SourceEnvelope> {
        (module == OTHER_MODULE)
            .then(|| SourceEnvelope::inline(SourceArtifactKind::Program, module, OTHER_SOURCE))
    }
}

fn function_id(name: &str) -> SemanticId {
    mncs_model::function_id(MODULE, name)
}

fn function_id_in(module: &str, name: &str) -> SemanticId {
    mncs_model::function_id(module, name)
}

fn host_integer(value: i128) -> HostExecutionValue {
    HostExecutionValue::Integer { value }
}

fn i64_value(value: i128) -> ExecutionValue {
    ExecutionValue::Integer {
        value,
        ty: IntegerType {
            bits: 64,
            signed: true,
        },
    }
}

fn open_seeded() -> Session {
    let seed = HostGenericSeedRequest::from_request(
        MODULE,
        "echo",
        &[ExecutionTypeArgument::Type { ty: "i64".into() }],
    );
    let artifact = Artifact::from_source_with_seeds(SOURCE, "mncs-research-bytecode", &[seed])
        .expect("compile identity-call fixture");
    Session::open(artifact).expect("open identity-call fixture")
}

fn open_multi_module_seeded() -> Session {
    let seed = HostGenericSeedRequest::from_request(
        MODULE,
        "echo",
        &[ExecutionTypeArgument::Type { ty: "i64".into() }],
    );
    let compiler = ReferenceCompiler::default();
    let front_end = compiler.front_end_with_resolver_and_seeds(
        SourceEnvelope::inline(
            SourceArtifactKind::Program,
            "probe.identity",
            MULTI_MODULE_SOURCE,
        ),
        &IdentityModuleResolver,
        &[seed],
    );
    assert!(front_end.is_valid(), "{:#?}", front_end.diagnostics);
    let program = front_end.program.expect("linked compiler program");
    let emit = [
        ArtifactRepresentation::Semantic,
        ArtifactRepresentation::Hir,
        ArtifactRepresentation::Ssa,
        ArtifactRepresentation::TargetLoweringPlan,
        ArtifactRepresentation::BackendArtifact,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let request = compiler
        .request_for_program_with_backend(&program, emit, "mncs-research-bytecode")
        .expect("research bytecode backend is registered");
    let compilation = compiler.compile(request, &program);
    assert!(
        matches!(
            compilation.status,
            mncs_model::CompilationStatus::Completed
                | mncs_model::CompilationStatus::CompletedWithUnresolvedObligations
        ),
        "{:#?}",
        compilation.diagnostics
    );
    let backend = compilation
        .emissions
        .backend
        .expect("backend artifact emitted");
    let bytes = serde_json::to_vec(&backend).expect("backend artifact serializes");
    Session::open(Artifact::from_json(&bytes).expect("verified multi-module artifact"))
        .expect("open multi-module session")
}

fn reference(session: &Session, name: &str) -> CallableReference {
    session
        .callable_reference(&function_id(name))
        .expect("compiler-owned function identity resolves")
}

fn returned_integer(output: &mncs_embed::CallOutput) -> i128 {
    assert_eq!(output.status, "returned", "{output:?}");
    match output.returned.as_slice() {
        [ExecutionValue::Integer { value, .. }] => *value,
        other => panic!("expected one integer result, got {other:?}"),
    }
}

#[test]
fn one_identity_dispatcher_invokes_heterogeneous_declarations() {
    let session = open_multi_module_seeded();
    let options = CallOptions::budgeted(8_192);
    let plus_one = reference(&session, "plus_one");
    let times_two = session
        .callable_reference(&function_id_in(OTHER_MODULE, "times_two"))
        .expect("the imported compiler-owned callable resolves in the same artifact");
    assert_eq!(plus_one.callable_identity, function_id("plus_one"));
    assert_eq!(
        times_two.callable_identity,
        function_id_in(OTHER_MODULE, "times_two")
    );
    let plus_output = session
        .call_identity_typed(&plus_one, vec![host_integer(40)], &options)
        .expect("identity dispatch invokes the first module");
    let times_output = session
        .call_identity_typed(&times_two, vec![host_integer(21)], &options)
        .expect("the same identity dispatcher invokes the second module");
    assert_eq!(returned_integer(&plus_output), 41);
    assert_eq!(returned_integer(&times_output), 42);
}

#[test]
fn generic_identity_dispatch_binds_runtime_type_arguments() {
    let session = open_seeded();
    let reference = reference(&session, "echo");
    let mut options = CallOptions::budgeted(8_192);
    options.type_arguments = vec![ExecutionTypeArgument::Type { ty: "i64".into() }];
    let output = session
        .call_identity_typed(&reference, vec![host_integer(73)], &options)
        .expect("seeded generic identity call succeeds");
    assert_eq!(returned_integer(&output), 73);
}

#[test]
fn identity_resolution_and_invocation_fail_closed() {
    let session = open_seeded();
    let original = reference(&session, "plus_one");
    let options = CallOptions::budgeted(8_192);

    let foreign_source = "mncs 0.18;\nmodule probe.foreign;\nfn alien(value: i64) -> (result: i64) { return value; }\n";
    let foreign_artifact = Artifact::from_source(foreign_source, "mncs-research-bytecode")
        .expect("compile a separate declaration");
    let foreign_session = Session::open(foreign_artifact).expect("open separate declaration");
    let foreign_reference = foreign_session
        .callable_reference(&mncs_model::function_id("probe.foreign", "alien"))
        .expect("foreign compiler identity resolves in its artifact");
    assert_eq!(
        session
            .callable_reference(&foreign_reference.callable_identity)
            .unwrap_err()
            .code,
        "unknown_callable_identity"
    );
    assert_eq!(
        session
            .call_identity(&foreign_reference, vec![i64_value(1)], &options)
            .unwrap_err()
            .code,
        "artifact_identity_mismatch"
    );

    let mut stale_declaration = original.clone();
    stale_declaration.declaration_identity = foreign_reference.declaration_identity.clone();
    assert_eq!(
        session
            .call_identity(&stale_declaration, vec![i64_value(1)], &options)
            .unwrap_err()
            .code,
        "stale_declaration_identity"
    );
    let mut stale_signature = original.clone();
    stale_signature.signature_identity = foreign_reference.signature_identity.clone();
    assert_eq!(
        session
            .call_identity(&stale_signature, vec![i64_value(1)], &options)
            .unwrap_err()
            .code,
        "signature_identity_mismatch"
    );
    let mut malformed_metadata = original.clone();
    malformed_metadata.test_case_identity = Some(foreign_reference.callable_identity.clone());
    assert_eq!(
        session
            .call_identity(&malformed_metadata, vec![i64_value(1)], &options)
            .unwrap_err()
            .code,
        "stale_declaration_identity"
    );

    let arity = session
        .call_identity(&original, Vec::new(), &options)
        .unwrap_err();
    assert_eq!(arity.code, "invalid_callable_arguments");
    assert!(arity.message.contains("expected 1 argument(s), received 0"));

    let wrong_type = ExecutionValue::Integer {
        value: 1,
        ty: IntegerType {
            bits: 32,
            signed: true,
        },
    };
    let error = session
        .call_identity(&original, vec![wrong_type], &options)
        .unwrap_err();
    assert_eq!(error.code, "invalid_callable_arguments");
    assert!(error.message.contains("argument 0"), "{error}");
}

#[test]
fn artifact_with_ambiguous_compiler_callable_identity_is_rejected() {
    let artifact = Artifact::from_source(SOURCE, "mncs-research-bytecode")
        .expect("compile the compiler-owned callable identities");
    let mut document: serde_json::Value =
        serde_json::from_slice(&artifact.to_json_bytes()).expect("artifact JSON");
    let bindings = document["callable_bindings"]
        .as_array_mut()
        .expect("compiler emits callable metadata");
    let compiler_binding = bindings[0].clone();
    bindings.push(compiler_binding);
    let bytes = serde_json::to_vec(&document).expect("tampered artifact JSON");
    let error = match Artifact::from_json(&bytes) {
        Ok(_) => panic!("ambiguous compiler identity metadata must not load"),
        Err(error) => error,
    };
    assert_eq!(error.code, "ambiguous_callable_identity");
}

#[test]
fn generic_identity_rejects_missing_wrong_and_uncompiled_arguments() {
    let session = open_seeded();
    let reference = reference(&session, "echo");
    let mut options = CallOptions::budgeted(8_192);
    let missing = session
        .call_identity_typed(&reference, vec![host_integer(5)], &options)
        .unwrap_err();
    assert_eq!(missing.code, "invalid_type_arguments");
    assert!(missing.message.contains("requires 1 generic type argument"));

    options.type_arguments = vec![ExecutionTypeArgument::Nat { value: 8 }];
    let wrong_kind = session
        .call_identity_typed(&reference, vec![host_integer(5)], &options)
        .unwrap_err();
    assert_eq!(wrong_kind.code, "invalid_type_arguments");
    assert!(wrong_kind.message.contains("expects Type"));

    options.type_arguments = vec![ExecutionTypeArgument::Type { ty: "i32".into() }];
    let uncompiled = session
        .call_identity_typed(&reference, vec![host_integer(5)], &options)
        .unwrap_err();
    assert_eq!(uncompiled.code, "bad_typed_arguments");
    assert!(uncompiled.message.contains("no compiled"));
}

#[test]
fn identity_and_named_calls_share_results_and_authority_checks() {
    let session = open_seeded();
    let plus_one_reference = reference(&session, "plus_one");
    let options = CallOptions::budgeted(8_192);
    let identity = session
        .call_identity_typed(&plus_one_reference, vec![host_integer(8)], &options)
        .expect("identity call");
    let named = session
        .call_typed(MODULE, "plus_one", vec![host_integer(8)], &options)
        .expect("named call");
    assert_eq!(identity.status, named.status);
    assert_eq!(identity.returned, named.returned);
    assert_eq!(identity.effects, named.effects);

    let mut stale_interface = options.clone();
    stale_interface.expected_interface_identity = Some("sha256:stale".to_owned());
    assert_eq!(
        session
            .call_identity_typed(&plus_one_reference, vec![host_integer(8)], &stale_interface,)
            .unwrap_err()
            .code,
        "stale_interface"
    );
    assert_eq!(
        session
            .call_typed(MODULE, "plus_one", vec![host_integer(8)], &stale_interface)
            .unwrap_err()
            .code,
        "stale_interface"
    );

    let protected = reference(&session, "read_clock");
    let denied = session
        .call_identity(&protected, Vec::new(), &options)
        .expect("effect refusal is a runtime observation");
    assert_eq!(denied.status, "unsupported");
    assert!(denied.returned.is_empty());

    let mut granted = options.clone();
    granted.grants = vec![Grant::time("ticker")];
    let allowed = session
        .call_identity(&protected, Vec::new(), &granted)
        .expect("granted effect call");
    assert_eq!(allowed.status, "returned");
    assert_eq!(allowed.effects.len(), 1);
    assert_eq!(allowed.effects[0].kind, "clock_read");
    assert_eq!(allowed.effects[0].capability, "ticker");
}

#[test]
fn retained_session_rejects_reference_from_another_artifact_revision() {
    let first = open_seeded();
    let old_reference = reference(&first, "plus_one");
    let changed_source = SOURCE.replace("value + 1", "value + 2");
    let seed = HostGenericSeedRequest::from_request(
        MODULE,
        "echo",
        &[ExecutionTypeArgument::Type { ty: "i64".into() }],
    );
    let changed =
        Artifact::from_source_with_seeds(&changed_source, "mncs-research-bytecode", &[seed])
            .expect("compile changed revision");
    let second = Session::open(changed).expect("open changed revision");
    assert_ne!(first.artifact_identity(), second.artifact_identity());
    assert_eq!(old_reference.callable_identity, function_id("plus_one"));
    assert_eq!(
        second
            .call_identity(&old_reference, vec![i64_value(10)], &CallOptions::default())
            .unwrap_err()
            .code,
        "artifact_identity_mismatch"
    );
}

#[test]
fn c_batch_boundary_dispatches_identity_and_generic_calls() {
    let seed = HostGenericSeedRequest::from_request(
        MODULE,
        "echo",
        &[ExecutionTypeArgument::Type { ty: "i64".into() }],
    );
    let artifact = Artifact::from_source_with_seeds(SOURCE, "mncs-research-bytecode", &[seed])
        .expect("compile identity-call fixture");
    let artifact_bytes = artifact.to_json_bytes();
    let session = Session::open(artifact).expect("open reference session");
    let plus_one = reference(&session, "plus_one");
    let echo = reference(&session, "echo");
    let requests = serde_json::json!([
        {
            "callable_reference": plus_one,
            "typed_args": [host_integer(12)],
            "step_budget": 8192
        },
        {
            "callable_reference": echo,
            "typed_args": [host_integer(39)],
            "type_arguments": [{"kind":"type", "type":"i64"}],
            "step_budget": 8192
        }
    ]);
    let request = CString::new(requests.to_string()).expect("request has no NUL");

    unsafe {
        let handle = mncs_embed::mncs_session_open(artifact_bytes.as_ptr(), artifact_bytes.len());
        assert!(!handle.is_null(), "C ABI session opens: {}", {
            CStr::from_ptr(mncs_embed::mncs_last_error()).to_string_lossy()
        });
        let response = mncs_embed::mncs_session_call_batch(handle, request.as_ptr());
        assert!(!response.is_null(), "C ABI batch succeeds: {}", {
            CStr::from_ptr(mncs_embed::mncs_last_error()).to_string_lossy()
        });
        let text = CStr::from_ptr(mncs_embed::mncs_response_text(response))
            .to_string_lossy()
            .into_owned();
        let outputs: serde_json::Value = serde_json::from_str(&text).expect("output JSON");
        assert_eq!(outputs[0]["status"], "returned");
        assert_eq!(outputs[0]["returned"][0]["integer"]["value"], 13);
        assert_eq!(outputs[1]["status"], "returned");
        assert_eq!(outputs[1]["returned"][0]["integer"]["value"], 39);
        mncs_embed::mncs_response_free(response);
        mncs_embed::mncs_session_close(handle);
    }
}
