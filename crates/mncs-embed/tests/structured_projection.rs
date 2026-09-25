use std::collections::BTreeMap;
use std::ffi::{CStr, CString};

use mncs_embed::{
    mncs_last_error, mncs_response_free, mncs_response_text, mncs_session_close,
    mncs_session_composite_types, mncs_session_open, mncs_session_project_value,
    mncs_session_serialize_value, Artifact, CallOptions, CompositeReference, Session,
};
use mncs_model::{ExecutionValue, HostExecutionValue, SemanticId};
use serde_json::{json, Value};

const SOURCE: &str = r#"
mncs 0.18;
module probe.structured_projection;

enum Phase { READY, BLOCKED }

record Point {
    x: i16,
    y: u16
}

record Parcel {
    active: bool,
    phase: Phase,
    payload: [byte; up_to 4],
    points: [Point; up_to 3]
}

record Counter {
    enabled: bool,
    value: u32
}

record Grid {
    rows: [[Point; up_to 2]; up_to 2]
}

fn echo(value: Parcel) -> (result: Parcel) {
    return value;
}
"#;

fn open_session(source: &str) -> Session {
    let artifact = Artifact::from_source(source, "mncs-research-bytecode")
        .expect("compile structured projection fixture");
    Session::open(artifact).expect("open structured projection artifact")
}

fn reference(session: &Session, name: &str) -> CompositeReference {
    session
        .composite_reference_by_name(name)
        .expect("unique compiler-owned type name")
}

fn identity(value: &ExecutionValue) -> &SemanticId {
    match value {
        ExecutionValue::Record { type_identity, .. }
        | ExecutionValue::Finite { type_identity, .. } => type_identity,
        other => panic!("expected nominal value, got {other:?}"),
    }
}

fn host_integer(value: i128) -> HostExecutionValue {
    HostExecutionValue::Integer { value }
}

fn host_point(x: i128, y: i128, type_name: &str) -> HostExecutionValue {
    HostExecutionValue::Record {
        type_name: type_name.to_owned(),
        fields: BTreeMap::from([
            ("x".to_owned(), host_integer(x)),
            ("y".to_owned(), host_integer(y)),
        ]),
    }
}

fn host_parcel() -> HostExecutionValue {
    HostExecutionValue::Record {
        type_name: "Parcel".to_owned(),
        fields: BTreeMap::from([
            (
                "active".to_owned(),
                HostExecutionValue::Boolean { value: true },
            ),
            (
                "phase".to_owned(),
                HostExecutionValue::Finite {
                    type_name: "Phase".to_owned(),
                    variant: "READY".to_owned(),
                    payload: BTreeMap::new(),
                },
            ),
            (
                "payload".to_owned(),
                HostExecutionValue::Sequence {
                    values: vec![
                        HostExecutionValue::Byte { value: 0 },
                        HostExecutionValue::Byte { value: 128 },
                        HostExecutionValue::Byte { value: 255 },
                    ],
                },
            ),
            (
                "points".to_owned(),
                HostExecutionValue::Sequence {
                    values: vec![host_point(-4, 11, "Point"), host_point(7, 13, "Point")],
                },
            ),
        ]),
    }
}

fn parcel_json() -> Value {
    json!({
        "active": true,
        "phase": "READY",
        "payload": [0, 128, 255],
        "points": [
            {"x": -4, "y": 11},
            {"x": 7, "y": 13}
        ]
    })
}

fn read_c_response(response: *mut mncs_embed::CallResponse) -> String {
    assert!(!response.is_null(), "C ABI failed: {:?}", unsafe {
        CStr::from_ptr(mncs_last_error())
    });
    let text = unsafe { CStr::from_ptr(mncs_response_text(response)) }
        .to_str()
        .expect("C ABI response is UTF-8")
        .to_owned();
    unsafe { mncs_response_free(response) };
    text
}

#[test]
fn compiler_owned_projection_materializes_records_and_nested_bounds() {
    let session = open_session(SOURCE);
    let parcel_ref = reference(&session, "Parcel");
    let point_ref = reference(&session, "Point");
    let counter_ref = reference(&session, "Counter");
    let grid_ref = reference(&session, "Grid");
    let phase_ref = reference(&session, "Phase");
    let phase_metadata = session
        .composite_types()
        .expect("compiler-owned composite metadata")
        .into_iter()
        .find(|item| item.name == "Phase")
        .expect("finite contract is listed");
    assert_eq!(phase_metadata.variants, ["READY", "BLOCKED"]);

    let parcel = session
        .project_value(&parcel_ref, &parcel_json())
        .expect("external record material projects from the artifact contract");
    let repeated = session
        .project_value(&parcel_ref, &parcel_json())
        .expect("same contract and input project deterministically");
    assert_eq!(parcel, repeated);
    assert_eq!(identity(&parcel), &parcel_ref.type_identity);
    assert_eq!(
        session
            .serialize_value(&parcel_ref, &parcel)
            .expect("publication uses compiler-owned fields and finite names"),
        parcel_json()
    );

    let point = session
        .project_value(&point_ref, &json!({"x": -7, "y": 15}))
        .expect("second record type uses the same projector");
    assert_eq!(identity(&point), &point_ref.type_identity);

    let counter = session
        .project_value(&counter_ref, &json!({"enabled": false, "value": 19}))
        .expect("another unrelated record uses the same projector");
    assert_eq!(identity(&counter), &counter_ref.type_identity);

    let grid = session
        .project_value(
            &grid_ref,
            &json!({
                "rows": [
                    [{"x": 1, "y": 2}],
                    [{"x": 3, "y": 4}, {"x": 5, "y": 6}]
                ]
            }),
        )
        .expect("nested bounded sequences of records project recursively");
    assert_eq!(identity(&grid), &grid_ref.type_identity);

    let ExecutionValue::Record { fields, .. } = &parcel else {
        panic!("parcel record expected")
    };
    let (_, phase) = fields
        .iter()
        .find(|(name, _)| name == "phase")
        .expect("phase field is present");
    let ExecutionValue::Finite { type_identity, .. } = phase else {
        panic!("finite phase expected, got {phase:?}")
    };
    assert_eq!(type_identity, &phase_ref.type_identity);
}

#[test]
fn compiler_contract_rejects_bad_shapes_values_bounds_and_identities() {
    let session = open_session(SOURCE);
    let parcel_ref = reference(&session, "Parcel");

    assert!(session
        .project_value(&parcel_ref, &json!({"active": true, "phase": "READY"}))
        .is_err()); // required field missing
    assert!(session
        .project_value(
            &parcel_ref,
            &json!({"active": true, "phase": "READY", "points": [], "extra": 1})
        )
        .is_err()); // exact projection rejects unknown fields
    assert!(session
        .project_value(
            &parcel_ref,
            &json!({"active": 1, "phase": "READY", "points": []})
        )
        .is_err()); // wrong primitive kind
    assert!(session
        .project_value(
            &parcel_ref,
            &json!({"active": true, "phase": "UNKNOWN", "points": []})
        )
        .is_err()); // unknown finite variant
    assert!(session
        .project_value(
            &parcel_ref,
            &json!({
                "active": true,
                "phase": "READY",
                "points": [
                    {"x": 0, "y": 0},
                    {"x": 1, "y": 1},
                    {"x": 2, "y": 2},
                    {"x": 3, "y": 3}
                ]
            })
        )
        .is_err()); // declared up_to bound is enforced
    assert!(session
        .project_value(
            &parcel_ref,
            &json!({
                "active": true,
                "phase": "READY",
                "payload": [256],
                "points": []
            })
        )
        .is_err()); // byte domain comes from the declared contract
    assert!(session
        .project_value(
            &parcel_ref,
            &json!({"active": true, "phase": "READY", "points": [{"x": 40000, "y": 1}]})
        )
        .is_err()); // compiler-declared i16 width is enforced
    assert_eq!(
        session
            .composite_reference(&SemanticId("mncs:foreign:record:Missing".to_owned()))
            .unwrap_err()
            .code,
        "unknown_composite_identity"
    );

    let foreign =
        open_session("mncs 0.18; module probe.foreign_projection; record Parcel { active: bool }");
    let foreign_ref = reference(&foreign, "Parcel");
    assert_eq!(
        session
            .project_value(&foreign_ref, &json!({"active": true}))
            .unwrap_err()
            .code,
        "artifact_identity_mismatch"
    );
}

#[test]
fn stale_revision_and_wrong_nested_nominal_material_fail_closed() {
    let session = open_session(SOURCE);
    let reference = reference(&session, "Parcel");
    let stale = open_session(&format!(
        "{SOURCE}\nfn revision_probe() -> (result: i64) {{ return 1; }}\n"
    ));
    assert_eq!(
        stale
            .project_value(&reference, &parcel_json())
            .unwrap_err()
            .code,
        "artifact_identity_mismatch"
    );

    let wrong_nested_type = HostExecutionValue::Record {
        type_name: "Parcel".to_owned(),
        fields: BTreeMap::from([
            (
                "active".to_owned(),
                HostExecutionValue::Boolean { value: true },
            ),
            (
                "phase".to_owned(),
                HostExecutionValue::Finite {
                    type_name: "Phase".to_owned(),
                    variant: "READY".to_owned(),
                    payload: BTreeMap::new(),
                },
            ),
            (
                "points".to_owned(),
                HostExecutionValue::Sequence {
                    values: vec![HostExecutionValue::Record {
                        type_name: "ForeignPoint".to_owned(),
                        fields: BTreeMap::from([
                            ("x".to_owned(), host_integer(1)),
                            ("y".to_owned(), host_integer(2)),
                        ]),
                    }],
                },
            ),
        ]),
    };
    assert!(session
        .project_host_value(&reference, &wrong_nested_type)
        .is_err());
}

#[test]
fn typed_calls_and_structured_projection_share_contract_authority() {
    let session = open_session(SOURCE);
    let reference = reference(&session, "Parcel");
    let projected = session
        .project_value(&reference, &parcel_json())
        .expect("generic structured projection succeeds");
    let callable = session
        .callable_reference(&mncs_model::function_id(
            "probe.structured_projection",
            "echo",
        ))
        .expect("compiler callable metadata resolves");
    let returned = session
        .call_identity_typed(
            &callable,
            vec![host_parcel()],
            &CallOptions::budgeted(8_192),
        )
        .expect("existing typed call uses the same artifact contracts");
    assert_eq!(returned.status, "returned", "{returned:?}");
    assert_eq!(returned.returned, vec![projected]);
    assert!(session.callable_bindings().iter().any(|binding| {
        binding.callable_identity == callable.callable_identity
            && binding.signature_identity == callable.signature_identity
    }));
}

#[test]
fn c_abi_exposes_artifact_bound_composite_metadata_and_projection() {
    let artifact = Artifact::from_source(SOURCE, "mncs-research-bytecode")
        .expect("compile C ABI projection fixture");
    let artifact_identity = artifact.artifact_identity().to_owned();
    let frozen = artifact.to_json_bytes();
    let handle = unsafe { mncs_session_open(frozen.as_ptr(), frozen.len()) };
    assert!(!handle.is_null(), "session open failed");

    let types: Value = serde_json::from_str(&read_c_response(unsafe {
        mncs_session_composite_types(handle)
    }))
    .expect("composite metadata JSON");
    let info = types
        .as_array()
        .expect("composite metadata is an array")
        .iter()
        .find(|item| item["name"] == "Parcel")
        .expect("Parcel metadata is present");
    assert_eq!(info["reference"]["artifact_identity"], artifact_identity);
    let phase_info = types
        .as_array()
        .expect("composite metadata is an array")
        .iter()
        .find(|item| item["name"] == "Phase")
        .expect("Phase metadata is present");
    assert_eq!(phase_info["variants"], json!(["READY", "BLOCKED"]));

    let request =
        CString::new(json!({"reference": info["reference"], "value": parcel_json()}).to_string())
            .expect("request contains no NUL");
    let projected: Value = serde_json::from_str(&read_c_response(unsafe {
        mncs_session_project_value(handle, request.as_ptr())
    }))
    .expect("projected value JSON");
    assert_eq!(projected["artifact_identity"], artifact_identity);
    assert_eq!(
        projected["type_identity"],
        info["reference"]["type_identity"]
    );
    assert_eq!(
        projected["value"]["record"]["type_identity"],
        info["reference"]["type_identity"]
    );

    let serialized_request = CString::new(
        json!({
            "reference": info["reference"],
            "value": projected["value"]
        })
        .to_string(),
    )
    .expect("request contains no NUL");
    let serialized: Value = serde_json::from_str(&read_c_response(unsafe {
        mncs_session_serialize_value(handle, serialized_request.as_ptr())
    }))
    .expect("serialized structured JSON");
    assert_eq!(serialized, parcel_json());

    unsafe { mncs_session_close(handle) };
}
