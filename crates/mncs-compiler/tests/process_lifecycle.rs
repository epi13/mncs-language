use mncs_compiler::{bundle::pinned_bundle, elaborate_program_with_resolver_and_modules};
use mncs_syntax::{parse, SourceArtifactKind, SourceEnvelope};

fn program() -> mncs_model::Program {
    let source = r#"
mncs 0.18;
module app.process_lifecycle;
use mncs.std.process.v1;

fn launch(request: ProcessRequest) -> (result: ProcessStartResult)
    capability process_capability
    effect process_run authorized_by process_capability
{
    return start(request);
}

fn inspect(handle: ProcessHandle) -> (result: ProcessObservation)
    capability process_capability
    effect process_run authorized_by process_capability
{
    return observe(handle);
}

fn request_stop(handle: ProcessHandle) -> (result: ProcessObservation)
    capability process_capability
    effect process_run authorized_by process_capability
{
    return cancel(handle);
}

fn collect(handle: ProcessHandle) -> (result: ProcessObservation)
    capability process_capability
    effect process_run authorized_by process_capability
{
    return reap(handle);
}
"#;
    let envelope = SourceEnvelope::inline(
        SourceArtifactKind::Program,
        "process-lifecycle-proof",
        source.to_owned(),
    );
    let parsed = parse(&envelope);
    assert!(parsed.is_valid(), "source parses: {:?}", parsed.diagnostics);
    let bundle = pinned_bundle().expect("standard-library pin verifies");
    let (result, _, _) = elaborate_program_with_resolver_and_modules(
        &parsed.ast.expect("parsed AST is present"),
        &bundle.resolver(),
    );
    result.unwrap_or_else(|errors| {
        panic!(
            "generic process lifecycle elaborates: {:?}",
            errors
                .iter()
                .map(|error| (&error.code, &error.message))
                .collect::<Vec<_>>()
        )
    })
}

#[test]
fn process_lifecycle_is_a_typed_capability_effect() {
    let program = program();
    assert!(program.validate().valid, "linked program validates");
    for name in ["launch", "inspect", "request_stop", "collect"] {
        let function = program
            .functions
            .iter()
            .find(|function| function.name == name)
            .unwrap_or_else(|| panic!("linked function {name} is present"));
        assert!(
            function.effects.iter().any(|effect| {
                effect.kind == "process_run" && effect.capability == "process_capability"
            }),
            "{name} retains explicit generic process authority"
        );
    }
}

#[test]
fn process_lifecycle_rejects_wrong_nominal_handle_input() {
    let source = r#"
mncs 0.18;
module app.process_lifecycle_wrong;
use mncs.std.process.v1;
record NotAProcessHandle { token: [byte; 32] }
fn inspect(handle: NotAProcessHandle) -> (result: ProcessObservation)
    capability process_capability
    effect process_run authorized_by process_capability
{
    return process_observe(handle);
}
"#;
    let envelope = SourceEnvelope::inline(
        SourceArtifactKind::Program,
        "process-lifecycle-wrong-handle",
        source.to_owned(),
    );
    let parsed = parse(&envelope);
    assert!(parsed.is_valid(), "source parses: {:?}", parsed.diagnostics);
    let bundle = pinned_bundle().expect("standard-library pin verifies");
    let (result, _, _) = elaborate_program_with_resolver_and_modules(
        &parsed.ast.expect("parsed AST is present"),
        &bundle.resolver(),
    );
    let errors = result.expect_err("a lookalike record cannot authorize cancellation");
    assert!(
        errors.iter().any(|error| error.code == "MNE286"),
        "wrong nominal type is identified: {:?}",
        errors
            .iter()
            .map(|error| (&error.code, &error.message))
            .collect::<Vec<_>>()
    );
}
