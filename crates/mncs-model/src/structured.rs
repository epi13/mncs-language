//! Generic bounded structured-artifact effects (Phase IV).
//!
//! The language value model is the schema authority.  This module only
//! supplies the external transport: a bounded canonical JSON envelope is
//! decoded against the nominal [`BodyType`] already present in the linked
//! [`Program`], or encoded from a value that has already passed that type
//! boundary.  Applications do not receive a dynamic JSON tree and no
//! application-specific decoder belongs here.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Number, Value as JsonValue};

use crate::canonical::sha256_hex;
use crate::execution::{ExecutionValue, HostGrant};
use crate::{BodyType, FiniteType, Program, RecordType, SemanticId, SequenceBound};

/// External artifact schema for the generic structured transport.
pub const STRUCTURED_ARTIFACT_SCHEMA_VERSION: &str = "mncs.structured-artifact/1";
/// Maximum bytes read or published for one structured artifact.
pub const STRUCTURED_ARTIFACT_MAX_BYTES: usize = 4 * 1024 * 1024;
/// Maximum decoded JSON nesting depth.
pub const STRUCTURED_MAX_DEPTH: usize = 64;
/// Maximum decoded JSON values in one artifact.
pub const STRUCTURED_MAX_NODES: usize = 32 * 1024;
/// Maximum members in one decoded object or array.
pub const STRUCTURED_MAX_COLLECTION: usize = 8192;
/// Maximum bytes in one decoded JSON string.
pub const STRUCTURED_MAX_STRING_BYTES: usize = 1024 * 1024;
/// Maximum relative artifact path carried by a source value.
pub const STRUCTURED_PATH_MAX_BYTES: usize = 1024;
/// Maximum schema/interface label carried by a source value.
pub const STRUCTURED_SCHEMA_MAX_BYTES: usize = 64;

/// Provenance of one structured artifact effect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredEffect {
    pub kind: String,
    pub target: String,
    pub provenance: String,
}

/// Structured effects have only two boundary meanings.  Malformed input is
/// an invalid request; an unavailable or failed host resource is a runtime
/// failure.  Neither case is coerced into an empty record or successful
/// publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredFail {
    InvalidRequest(String),
    RuntimeFailure(String),
}

impl StructuredFail {
    fn invalid(message: impl Into<String>) -> Self {
        Self::InvalidRequest(message.into())
    }

    fn runtime(message: impl Into<String>) -> Self {
        Self::RuntimeFailure(message.into())
    }
}

#[derive(Debug, Clone)]
struct ArtifactPath {
    root: PathBuf,
    target: PathBuf,
    display: String,
}

/// Read and decode one external artifact from an explicit relative path
/// under the granted root.  `expected` is the nominal source-level result
/// type; `schema` is the contract's external schema revision.  Both are
/// checked before the typed value is returned.
pub fn structured_read_value(
    program: &Program,
    expected: &BodyType,
    grant: &HostGrant,
    path: &[u8],
    schema: &[u8],
) -> Result<(ExecutionValue, StructuredEffect), StructuredFail> {
    let schema = schema_label(schema)?;
    let artifact_path = resolve_artifact_path(grant, path, false)?;
    let mut file = open_nofollow(&artifact_path.target, false, false).map_err(|error| {
        if is_symlink_refusal(&error) {
            StructuredFail::invalid(format!(
                "structured_read artifact {:?} is a symlink; refusing",
                artifact_path.display
            ))
        } else {
            StructuredFail::runtime(format!(
                "structured_read artifact {:?} could not be opened: {error}",
                artifact_path.display
            ))
        }
    })?;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take((STRUCTURED_ARTIFACT_MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            StructuredFail::runtime(format!(
                "structured_read artifact {:?} could not be read: {error}",
                artifact_path.display
            ))
        })?;
    if bytes.len() > STRUCTURED_ARTIFACT_MAX_BYTES {
        return Err(StructuredFail::invalid(format!(
            "structured_read artifact {:?} exceeds the {STRUCTURED_ARTIFACT_MAX_BYTES}-byte bound",
            artifact_path.display
        )));
    }

    let value = decode_document(program, expected, &bytes, &schema)?;
    let digest = sha256_hex(&bytes);
    Ok((
        value,
        StructuredEffect {
            kind: "structured_read".to_owned(),
            target: artifact_path.display.clone(),
            provenance: format!(
                "grant:{} path:{} schema:{} sha256:{}",
                grant.locator, artifact_path.display, schema, digest
            ),
        },
    ))
}

/// Encode one already-typed value into the generic external artifact
/// envelope.  With `realize == false` the document is fully validated and
/// its byte count is returned, but no filesystem mutation occurs; this is
/// the record-only observation mode used by layered execution.
pub fn structured_write_value(
    program: &Program,
    expected: &BodyType,
    value: &ExecutionValue,
    grant: &HostGrant,
    path: &[u8],
    schema: &[u8],
    realize: bool,
) -> Result<(ExecutionValue, StructuredEffect), StructuredFail> {
    let schema = schema_label(schema)?;
    let artifact_path = resolve_artifact_path(grant, path, true)?;
    // Contract-shaped schema versions use the family-facing JSON projection
    // (for example, finite variants are strings and exact digests are hex).
    // The generic envelope deliberately retains the typed representation so
    // its interface/value digest can be checked before nominal decoding.
    let json_value = encode_typed_value(
        program,
        expected,
        value,
        0,
        schema != STRUCTURED_ARTIFACT_SCHEMA_VERSION,
    )?;
    let envelope = if schema == STRUCTURED_ARTIFACT_SCHEMA_VERSION {
        let value_digest = sha256_hex(&serde_json::to_vec(&json_value).map_err(|error| {
            StructuredFail::invalid(format!(
                "structured_write value digest encoding failed: {error}"
            ))
        })?);
        structured_envelope(expected, &schema, json_value, &value_digest)?
    } else {
        structured_external_document(&schema, json_value)?
    };
    let bytes = serde_json::to_vec(&envelope).map_err(|error| {
        StructuredFail::invalid(format!(
            "structured_write could not encode the typed value: {error}"
        ))
    })?;
    if bytes.len() > STRUCTURED_ARTIFACT_MAX_BYTES {
        return Err(StructuredFail::invalid(format!(
            "structured_write artifact {:?} exceeds the {STRUCTURED_ARTIFACT_MAX_BYTES}-byte bound",
            artifact_path.display
        )));
    }

    if realize {
        atomic_write(&artifact_path, &bytes)?;
    }

    let digest = sha256_hex(&bytes);
    Ok((
        ExecutionValue::Integer {
            value: bytes.len() as i128,
            ty: crate::IntegerType {
                bits: 64,
                signed: false,
            },
        },
        StructuredEffect {
            kind: "structured_write".to_owned(),
            target: artifact_path.display.clone(),
            provenance: format!(
                "grant:{} path:{} schema:{} bytes:{} sha256:{}",
                grant.locator,
                artifact_path.display,
                schema,
                bytes.len(),
                digest
            ),
        },
    ))
}

fn schema_label(schema: &[u8]) -> Result<String, StructuredFail> {
    if schema.is_empty() || schema.len() > STRUCTURED_SCHEMA_MAX_BYTES {
        return Err(StructuredFail::invalid(format!(
            "structured schema label must be 1..={STRUCTURED_SCHEMA_MAX_BYTES} bytes"
        )));
    }
    let label = String::from_utf8(schema.to_vec())
        .map_err(|_| StructuredFail::invalid("structured schema label is not UTF-8"))?;
    if label.bytes().any(|byte| byte == 0) {
        return Err(StructuredFail::invalid(
            "structured schema label contains NUL",
        ));
    }
    Ok(label)
}

fn resolve_artifact_path(
    grant: &HostGrant,
    path: &[u8],
    write: bool,
) -> Result<ArtifactPath, StructuredFail> {
    if grant.locator.is_empty() {
        return Err(StructuredFail::invalid(
            "structured artifact effect requires a non-empty granted root locator",
        ));
    }
    if path.is_empty() || path.len() > STRUCTURED_PATH_MAX_BYTES {
        return Err(StructuredFail::invalid(format!(
            "structured artifact path must be 1..={STRUCTURED_PATH_MAX_BYTES} bytes"
        )));
    }
    let display = String::from_utf8(path.to_vec())
        .map_err(|_| StructuredFail::invalid("structured artifact path is not UTF-8"))?;
    if display.bytes().any(|byte| byte == 0 || byte == b'\\') {
        return Err(StructuredFail::invalid(
            "structured artifact path contains NUL or a backslash",
        ));
    }
    let relative = Path::new(&display);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(StructuredFail::invalid(
            "structured artifact path must be a relative path of normal components; traversal is refused",
        ));
    }
    let root = std::fs::canonicalize(&grant.locator).map_err(|error| {
        StructuredFail::invalid(format!(
            "structured grant root {:?} is not readable: {error}",
            grant.locator
        ))
    })?;
    if !root.is_dir() {
        return Err(StructuredFail::invalid(format!(
            "structured grant root {:?} is not a directory",
            grant.locator
        )));
    }
    // Canonicalizing the parent alone would follow an intermediate symlink
    // that points back inside the root. Walk every existing component first:
    // the structured effect has no-follow semantics for the complete
    // relative path, not only for its final file.
    let components = relative.components().collect::<Vec<_>>();
    let mut cursor = root.clone();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            return Err(StructuredFail::invalid(
                "structured artifact path contains a non-normal component",
            ));
        };
        cursor.push(name);
        match std::fs::symlink_metadata(&cursor) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(StructuredFail::invalid(format!(
                    "structured artifact path component {:?} is a symlink; refusing",
                    display
                )))
            }
            Ok(metadata) if index + 1 < components.len() && !metadata.is_dir() => {
                return Err(StructuredFail::invalid(format!(
                    "structured artifact path component {:?} is not a directory",
                    display
                )))
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(StructuredFail::runtime(format!(
                    "structured artifact path component {:?} could not be inspected: {error}",
                    display
                )))
            }
        }
    }
    let candidate = root.join(relative);
    let parent = candidate.parent().ok_or_else(|| {
        StructuredFail::invalid("structured artifact path has no containing directory")
    })?;
    let parent = std::fs::canonicalize(parent).map_err(|error| {
        let kind = if write {
            "structured_write artifact parent"
        } else {
            "structured_read artifact parent"
        };
        StructuredFail::runtime(format!("{kind} {:?} is unavailable: {error}", display))
    })?;
    if !parent.starts_with(&root) {
        return Err(StructuredFail::invalid(
            "structured artifact parent resolves outside the granted root",
        ));
    }
    let file_name = relative.file_name().ok_or_else(|| {
        StructuredFail::invalid("structured artifact path has no final file component")
    })?;
    let target = parent.join(file_name);
    if let Ok(metadata) = std::fs::symlink_metadata(&target) {
        if metadata.file_type().is_symlink() {
            return Err(StructuredFail::invalid(format!(
                "structured artifact {:?} is a symlink; refusing",
                display
            )));
        }
        if metadata.is_dir() {
            return Err(StructuredFail::invalid(format!(
                "structured artifact {:?} names a directory; refusing",
                display
            )));
        }
    }
    Ok(ArtifactPath {
        root,
        target,
        display,
    })
}

fn decode_document(
    program: &Program,
    expected: &BodyType,
    bytes: &[u8],
    schema: &str,
) -> Result<ExecutionValue, StructuredFail> {
    let json: JsonValue = serde_json::from_slice(bytes).map_err(|error| {
        StructuredFail::invalid(format!("structured artifact is not valid JSON: {error}"))
    })?;
    enforce_json_bounds(&json)?;
    let JsonValue::Object(mut envelope) = json else {
        return Err(StructuredFail::invalid(
            "structured artifact must be a JSON object",
        ));
    };
    let expected_envelope_fields = [
        "interface_identity",
        "schema_version",
        "value",
        "value_digest",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect::<BTreeSet<_>>();
    let actual_envelope_fields = envelope.keys().cloned().collect::<BTreeSet<_>>();
    if actual_envelope_fields != expected_envelope_fields {
        let actual_schema = envelope
            .remove("schema_version")
            .and_then(|value| value.as_str().map(str::to_owned))
            .ok_or_else(|| {
                StructuredFail::invalid(
                    "structured artifact is neither a generic envelope nor a schema-versioned document",
                )
            })?;
        if actual_schema != schema {
            return Err(StructuredFail::invalid(format!(
                "structured artifact schema_version {actual_schema:?} does not match requested {schema:?}"
            )));
        }
        return decode_typed_value(program, expected, &JsonValue::Object(envelope), 0, true);
    }
    let actual_schema = envelope
        .get("schema_version")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| StructuredFail::invalid("structured artifact is missing schema_version"))?;
    if actual_schema != schema {
        return Err(StructuredFail::invalid(format!(
            "structured artifact schema_version {actual_schema:?} does not match requested {schema:?}"
        )));
    }
    let actual_interface = envelope
        .get("interface_identity")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| {
            StructuredFail::invalid("structured artifact is missing interface_identity")
        })?;
    let expected_interface = expected.canonical_identity();
    if actual_interface != expected_interface {
        return Err(StructuredFail::invalid(format!(
            "structured artifact interface_identity {actual_interface:?} does not match expected {expected_interface:?}"
        )));
    }
    let value = envelope
        .get("value")
        .ok_or_else(|| StructuredFail::invalid("structured artifact is missing value"))?;
    let expected_value_digest = envelope
        .get("value_digest")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| StructuredFail::invalid("structured artifact is missing value_digest"))?;
    let actual_value_digest = sha256_hex(&serde_json::to_vec(value).map_err(|error| {
        StructuredFail::invalid(format!(
            "structured artifact value digest encoding failed: {error}"
        ))
    })?);
    if expected_value_digest != actual_value_digest {
        return Err(StructuredFail::invalid(
            "structured artifact value_digest does not match the encoded value",
        ));
    }
    decode_typed_value(program, expected, value, 0, false)
}

fn enforce_json_bounds(value: &JsonValue) -> Result<(), StructuredFail> {
    fn walk(value: &JsonValue, depth: usize, nodes: &mut usize) -> Result<(), StructuredFail> {
        *nodes = nodes.saturating_add(1);
        if *nodes > STRUCTURED_MAX_NODES {
            return Err(StructuredFail::invalid(format!(
                "structured artifact exceeds the {STRUCTURED_MAX_NODES}-node bound"
            )));
        }
        if depth > STRUCTURED_MAX_DEPTH {
            return Err(StructuredFail::invalid(format!(
                "structured artifact exceeds the {STRUCTURED_MAX_DEPTH}-level nesting bound"
            )));
        }
        match value {
            JsonValue::Array(items) => {
                if items.len() > STRUCTURED_MAX_COLLECTION {
                    return Err(StructuredFail::invalid(format!(
                        "structured artifact array exceeds the {STRUCTURED_MAX_COLLECTION}-item bound"
                    )));
                }
                for item in items {
                    walk(item, depth + 1, nodes)?;
                }
            }
            JsonValue::Object(fields) => {
                if fields.len() > STRUCTURED_MAX_COLLECTION {
                    return Err(StructuredFail::invalid(format!(
                        "structured artifact object exceeds the {STRUCTURED_MAX_COLLECTION}-field bound"
                    )));
                }
                for (name, item) in fields {
                    if name.len() > STRUCTURED_MAX_STRING_BYTES {
                        return Err(StructuredFail::invalid(
                            "structured artifact object key exceeds the string bound",
                        ));
                    }
                    walk(item, depth + 1, nodes)?;
                }
            }
            JsonValue::String(text) if text.len() > STRUCTURED_MAX_STRING_BYTES => {
                return Err(StructuredFail::invalid(
                    "structured artifact string exceeds the string bound",
                ));
            }
            _ => {}
        }
        Ok(())
    }
    walk(value, 0, &mut 0)
}

fn decode_typed_value(
    program: &Program,
    expected: &BodyType,
    value: &JsonValue,
    depth: usize,
    external: bool,
) -> Result<ExecutionValue, StructuredFail> {
    if depth > STRUCTURED_MAX_DEPTH {
        return Err(StructuredFail::invalid(
            "typed structured value exceeds the nesting bound",
        ));
    }
    match expected {
        BodyType::Bool => value
            .as_bool()
            .map(|value| ExecutionValue::Boolean { value })
            .ok_or_else(|| StructuredFail::invalid("typed bool value is not a JSON boolean")),
        BodyType::Integer(ty) => {
            let JsonValue::Number(number) = value else {
                return Err(StructuredFail::invalid(
                    "typed integer value is not a JSON number",
                ));
            };
            let integer = number.to_string().parse::<i128>().map_err(|_| {
                StructuredFail::invalid("typed integer value is not an integral i128")
            })?;
            if !ty.signed && integer < 0 {
                return Err(StructuredFail::invalid(
                    "unsigned typed integer is negative",
                ));
            }
            if !integer_fits(ty, integer) {
                return Err(StructuredFail::invalid(format!(
                    "typed integer value {integer} does not fit {}",
                    ty.bits
                )));
            }
            Ok(ExecutionValue::Integer {
                value: integer,
                ty: *ty,
            })
        }
        BodyType::Float(ty) => {
            let float = value
                .as_f64()
                .filter(|float| float.is_finite())
                .ok_or_else(|| {
                    StructuredFail::invalid("typed float is not a finite JSON number")
                })?;
            Ok(ExecutionValue::Float {
                bits: float.to_bits(),
                ty: *ty,
            })
        }
        BodyType::Byte => {
            let number = value.as_u64().ok_or_else(|| {
                StructuredFail::invalid("typed byte is not an unsigned JSON number")
            })?;
            if number > u64::from(u8::MAX) {
                return Err(StructuredFail::invalid("typed byte is outside 0..=255"));
            }
            Ok(ExecutionValue::Byte {
                value: i128::from(number),
            })
        }
        BodyType::Sequence { element, bound } => {
            let values = decode_sequence(program, element, bound, value, depth, external)?;
            Ok(ExecutionValue::Sequence {
                values: values.into(),
            })
        }
        BodyType::Vector { element, lanes } => {
            let JsonValue::Array(items) = value else {
                return Err(StructuredFail::invalid("typed vector is not a JSON array"));
            };
            if items.len() != *lanes as usize {
                return Err(StructuredFail::invalid(
                    "typed vector lane count does not match",
                ));
            }
            let mut values = Vec::with_capacity(items.len());
            for item in items {
                values.push(decode_typed_value(
                    program,
                    element,
                    item,
                    depth + 1,
                    external,
                )?);
            }
            Ok(ExecutionValue::Vector {
                values: values.into(),
            })
        }
        BodyType::Mask { lanes } => {
            let JsonValue::Array(items) = value else {
                return Err(StructuredFail::invalid("typed mask is not a JSON array"));
            };
            if items.len() != *lanes as usize {
                return Err(StructuredFail::invalid(
                    "typed mask lane count does not match",
                ));
            }
            let mut lanes_value = Vec::with_capacity(items.len());
            for item in items {
                lanes_value.push(item.as_bool().ok_or_else(|| {
                    StructuredFail::invalid("typed mask lane is not a JSON boolean")
                })?);
            }
            Ok(ExecutionValue::Mask {
                lanes: lanes_value.into(),
            })
        }
        BodyType::Finite { identity, .. } => {
            decode_finite(program, identity, value, depth, external)
        }
        BodyType::Record { identity, .. } => {
            decode_record(program, identity, value, depth, external)
        }
        BodyType::Named(name) => Err(StructuredFail::invalid(format!(
            "structured decoder refuses unresolved named type {name:?}"
        ))),
        BodyType::GenericParam { name } => Err(StructuredFail::invalid(format!(
            "structured decoder refuses unspecialized generic type {name:?}"
        ))),
    }
}

fn decode_sequence(
    program: &Program,
    element: &BodyType,
    bound: &SequenceBound,
    value: &JsonValue,
    depth: usize,
    external: bool,
) -> Result<Vec<ExecutionValue>, StructuredFail> {
    let mut byte_string = None;
    let items: Vec<JsonValue> = if *element == BodyType::Byte {
        match value {
            JsonValue::String(text) => {
                byte_string = Some(text.as_bytes().to_vec());
                Vec::new()
            }
            JsonValue::Array(items) => items.clone(),
            _ => {
                return Err(StructuredFail::invalid(
                    "typed byte sequence is neither a JSON string nor array",
                ));
            }
        }
    } else {
        value
            .as_array()
            .cloned()
            .ok_or_else(|| StructuredFail::invalid("typed sequence is not a JSON array"))?
    };
    let values = if let Some(bytes) = byte_string {
        decode_byte_string(bytes, bound)?
    } else {
        check_sequence_length(items.len(), bound)?;
        let mut values = Vec::with_capacity(items.len());
        for item in &items {
            values.push(decode_typed_value(
                program,
                element,
                item,
                depth + 1,
                external,
            )?);
        }
        values
    };
    Ok(values)
}

fn decode_byte_string(
    bytes: Vec<u8>,
    bound: &SequenceBound,
) -> Result<Vec<ExecutionValue>, StructuredFail> {
    let bytes = if matches!(bound, SequenceBound::Exact(32)) && bytes.len() == 64 {
        let text = String::from_utf8(bytes)
            .map_err(|_| StructuredFail::invalid("fixed digest string is not UTF-8"))?;
        if !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(StructuredFail::invalid(
                "fixed 32-byte identity must use a hexadecimal string or byte array",
            ));
        }
        (0..32)
            .map(|index| {
                u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
                    .map_err(|_| StructuredFail::invalid("fixed identity contains invalid hex"))
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        bytes
    };
    check_sequence_length(bytes.len(), bound)?;
    Ok(bytes
        .into_iter()
        .map(|value| ExecutionValue::Byte {
            value: i128::from(value),
        })
        .collect())
}

fn check_sequence_length(length: usize, bound: &SequenceBound) -> Result<(), StructuredFail> {
    match bound {
        SequenceBound::Exact(expected) if length != *expected as usize => {
            Err(StructuredFail::invalid(format!(
                "typed sequence length {length} does not match exact {expected}"
            )))
        }
        SequenceBound::UpTo(max) if length > *max as usize => Err(StructuredFail::invalid(
            format!("typed sequence length {length} exceeds up_to {max}"),
        )),
        SequenceBound::Param(name) | SequenceBound::UpToParam(name) => {
            Err(StructuredFail::invalid(format!(
                "structured decoder refuses unresolved sequence bound {name:?}"
            )))
        }
        _ => Ok(()),
    }
}

fn decode_record(
    program: &Program,
    identity: &SemanticId,
    value: &JsonValue,
    depth: usize,
    external: bool,
) -> Result<ExecutionValue, StructuredFail> {
    let record = find_record(program, identity)?;
    let JsonValue::Object(fields) = value else {
        return Err(StructuredFail::invalid(format!(
            "typed record {} is not a JSON object",
            record.name
        )));
    };
    let expected_names = record
        .fields
        .iter()
        .map(|field| field.name.clone())
        .collect::<BTreeSet<_>>();
    let actual_names = fields.keys().cloned().collect::<BTreeSet<_>>();
    if (!external && actual_names != expected_names)
        || (external && !actual_names.is_superset(&expected_names))
    {
        return Err(StructuredFail::invalid(format!(
            "typed record {} fields do not exactly match its contract",
            record.name
        )));
    }
    let mut decoded = Vec::with_capacity(record.fields.len());
    for field in &record.fields {
        let field_type = BodyType::from_program_in_module(
            program,
            &field.field_type,
            &record.identity.declaring_module().unwrap_or_default(),
        );
        let field_value = decode_typed_value(
            program,
            &field_type,
            fields.get(&field.name).expect("field set checked"),
            depth + 1,
            external,
        )?;
        decoded.push((field.name.clone(), field_value));
    }
    Ok(ExecutionValue::Record {
        type_identity: identity.clone(),
        name: record.name.clone(),
        fields: decoded.into(),
    })
}

fn decode_finite(
    program: &Program,
    identity: &SemanticId,
    value: &JsonValue,
    depth: usize,
    external: bool,
) -> Result<ExecutionValue, StructuredFail> {
    let finite = find_finite(program, identity)?;
    if external {
        if let Some(variant_name) = value.as_str() {
            let variant = finite
                .variants
                .iter()
                .find(|variant| variant.name == variant_name)
                .ok_or_else(|| StructuredFail::invalid("typed finite names an unknown variant"))?;
            if !variant.payload.is_empty() {
                return Err(StructuredFail::invalid(
                    "external finite string cannot omit a non-empty payload",
                ));
            }
            return Ok(ExecutionValue::Finite {
                type_identity: identity.clone(),
                variant_identity: variant.identity.clone(),
                discriminant: variant.discriminant,
                payload: Vec::new().into(),
            });
        }
    }
    let JsonValue::Object(fields) = value else {
        return Err(StructuredFail::invalid(format!(
            "typed finite {} is not a JSON object",
            finite.name
        )));
    };
    let variant_name = fields
        .get("variant")
        .and_then(JsonValue::as_str)
        .ok_or_else(|| StructuredFail::invalid("typed finite is missing variant"))?;
    let variant = finite
        .variants
        .iter()
        .find(|variant| variant.name == variant_name)
        .ok_or_else(|| StructuredFail::invalid("typed finite names an unknown variant"))?;
    let payload = fields
        .get("payload")
        .ok_or_else(|| StructuredFail::invalid("typed finite is missing payload"))?;
    let JsonValue::Object(payload_values) = payload else {
        return Err(StructuredFail::invalid(
            "typed finite payload is not an object",
        ));
    };
    let expected_names = variant
        .payload
        .iter()
        .map(|field| field.name.clone())
        .collect::<BTreeSet<_>>();
    let actual_names = payload_values.keys().cloned().collect::<BTreeSet<_>>();
    if actual_names != expected_names {
        return Err(StructuredFail::invalid(
            "typed finite payload fields do not exactly match its contract",
        ));
    }
    let mut decoded = Vec::with_capacity(variant.payload.len());
    for field in &variant.payload {
        let field_type = BodyType::from_program_in_module(
            program,
            &field.field_type,
            &finite.identity.declaring_module().unwrap_or_default(),
        );
        let field_value = decode_typed_value(
            program,
            &field_type,
            payload_values
                .get(&field.name)
                .expect("payload set checked"),
            depth + 1,
            external,
        )?;
        decoded.push((field.name.clone(), field_value));
    }
    Ok(ExecutionValue::Finite {
        type_identity: identity.clone(),
        variant_identity: variant.identity.clone(),
        discriminant: variant.discriminant,
        payload: decoded.into(),
    })
}

pub(crate) fn encode_typed_value(
    program: &Program,
    expected: &BodyType,
    value: &ExecutionValue,
    depth: usize,
    external: bool,
) -> Result<JsonValue, StructuredFail> {
    if depth > STRUCTURED_MAX_DEPTH {
        return Err(StructuredFail::invalid(
            "typed structured value exceeds the nesting bound",
        ));
    }
    match (expected, value) {
        (BodyType::Bool, ExecutionValue::Boolean { value }) => Ok(JsonValue::Bool(*value)),
        (BodyType::Integer(expected), ExecutionValue::Integer { value, ty })
            if expected == ty && integer_fits(expected, *value) =>
        {
            Number::from_i128(*value)
                .map(JsonValue::Number)
                .ok_or_else(|| {
                    StructuredFail::invalid("typed integer cannot be represented as JSON")
                })
        }
        (BodyType::Float(expected), ExecutionValue::Float { bits, ty }) if expected == ty => {
            let value = f64::from_bits(*bits);
            if value.is_finite() {
                Number::from_f64(value)
                    .map(JsonValue::Number)
                    .ok_or_else(|| StructuredFail::invalid("typed float is not JSON finite"))
            } else {
                Err(StructuredFail::invalid("typed float is non-finite"))
            }
        }
        (BodyType::Byte, ExecutionValue::Byte { value }) if (0..=255).contains(value) => {
            Ok(JsonValue::Number(Number::from(*value as u64)))
        }
        (BodyType::Sequence { element, bound }, ExecutionValue::Sequence { values }) => {
            check_sequence_length(values.len(), bound)?;
            if **element == BodyType::Byte {
                encode_byte_sequence(values, bound)
            } else {
                let mut items = Vec::with_capacity(values.len());
                for item in values.iter() {
                    items.push(encode_typed_value(
                        program,
                        element,
                        item,
                        depth + 1,
                        external,
                    )?);
                }
                Ok(JsonValue::Array(items))
            }
        }
        (BodyType::Vector { element, lanes }, ExecutionValue::Vector { values })
            if values.len() == *lanes as usize =>
        {
            let mut items = Vec::with_capacity(values.len());
            for item in values.iter() {
                items.push(encode_typed_value(
                    program,
                    element,
                    item,
                    depth + 1,
                    external,
                )?);
            }
            Ok(JsonValue::Array(items))
        }
        (BodyType::Mask { lanes }, ExecutionValue::Mask { lanes: values })
            if values.len() == *lanes as usize =>
        {
            Ok(JsonValue::Array(
                values.iter().copied().map(JsonValue::Bool).collect(),
            ))
        }
        (
            BodyType::Record { identity, .. },
            ExecutionValue::Record {
                type_identity,
                fields,
                ..
            },
        ) if identity == type_identity => {
            let record = find_record(program, identity)?;
            let actual = fields
                .iter()
                .map(|(name, _)| name.clone())
                .collect::<BTreeSet<_>>();
            let expected = record
                .fields
                .iter()
                .map(|field| field.name.clone())
                .collect::<BTreeSet<_>>();
            if actual != expected {
                return Err(StructuredFail::invalid(format!(
                    "typed record {} fields do not exactly match its contract",
                    record.name
                )));
            }
            let mut object = Map::new();
            for field in &record.fields {
                let field_value = fields
                    .iter()
                    .find(|(name, _)| name == &field.name)
                    .map(|(_, value)| value)
                    .expect("field set checked");
                let field_type = BodyType::from_program_in_module(
                    program,
                    &field.field_type,
                    &record.identity.declaring_module().unwrap_or_default(),
                );
                object.insert(
                    field.name.clone(),
                    encode_typed_value(program, &field_type, field_value, depth + 1, external)?,
                );
            }
            Ok(JsonValue::Object(object))
        }
        (
            BodyType::Finite { identity, .. },
            ExecutionValue::Finite {
                type_identity,
                variant_identity,
                payload,
                ..
            },
        ) if identity == type_identity => {
            let finite = find_finite(program, identity)?;
            let variant = finite
                .variants
                .iter()
                .find(|variant| &variant.identity == variant_identity)
                .ok_or_else(|| {
                    StructuredFail::invalid("typed finite variant identity is unknown")
                })?;
            if external && variant.payload.is_empty() {
                return Ok(JsonValue::String(variant.name.clone()));
            }
            let mut payload_object = Map::new();
            let actual = payload
                .iter()
                .map(|(name, _)| name.clone())
                .collect::<BTreeSet<_>>();
            let expected = variant
                .payload
                .iter()
                .map(|field| field.name.clone())
                .collect::<BTreeSet<_>>();
            if actual != expected {
                return Err(StructuredFail::invalid(
                    "typed finite payload fields do not exactly match its contract",
                ));
            }
            for field in &variant.payload {
                let field_value = payload
                    .iter()
                    .find(|(name, _)| name == &field.name)
                    .map(|(_, value)| value)
                    .expect("payload set checked");
                let field_type = BodyType::from_program_in_module(
                    program,
                    &field.field_type,
                    &finite.identity.declaring_module().unwrap_or_default(),
                );
                payload_object.insert(
                    field.name.clone(),
                    encode_typed_value(program, &field_type, field_value, depth + 1, external)?,
                );
            }
            let mut object = Map::new();
            object.insert(
                "variant".to_owned(),
                JsonValue::String(variant.name.clone()),
            );
            object.insert("payload".to_owned(), JsonValue::Object(payload_object));
            Ok(JsonValue::Object(object))
        }
        (BodyType::Named(name), _) => Err(StructuredFail::invalid(format!(
            "structured encoder refuses unresolved named type {name:?}"
        ))),
        (BodyType::GenericParam { name }, _) => Err(StructuredFail::invalid(format!(
            "structured encoder refuses unspecialized generic type {name:?}"
        ))),
        _ => Err(StructuredFail::invalid(format!(
            "typed value does not satisfy {}",
            expected.semantic_name()
        ))),
    }
}

/// Encode a resolved value using the family-facing canonical contract
/// projection. This is shared by structured publication and
/// `structured_digest`, so a typed artifact has one external representation
/// whether it is being written or identity-hashed.
pub(crate) fn canonical_external_value(
    program: &Program,
    expected: &BodyType,
    value: &ExecutionValue,
) -> Result<JsonValue, String> {
    encode_typed_value(program, expected, value, 0, true).map_err(|error| match error {
        StructuredFail::InvalidRequest(message) | StructuredFail::RuntimeFailure(message) => {
            message
        }
    })
}

fn encode_byte_sequence(
    values: &[ExecutionValue],
    bound: &SequenceBound,
) -> Result<JsonValue, StructuredFail> {
    let mut bytes = Vec::with_capacity(values.len());
    for value in values {
        let ExecutionValue::Byte { value } = value else {
            return Err(StructuredFail::invalid(
                "byte sequence contains a non-byte value",
            ));
        };
        bytes.push(
            u8::try_from(*value).map_err(|_| StructuredFail::invalid("byte is outside 0..=255"))?,
        );
    }
    if matches!(bound, SequenceBound::Exact(32)) {
        return Ok(JsonValue::String(
            bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        ));
    }
    match String::from_utf8(bytes.clone()) {
        Ok(text) => Ok(JsonValue::String(text)),
        Err(_) => Ok(JsonValue::Array(
            bytes
                .into_iter()
                .map(|byte| JsonValue::Number(Number::from(byte)))
                .collect(),
        )),
    }
}

fn integer_fits(ty: &crate::IntegerType, value: i128) -> bool {
    if ty.bits == 0 || ty.bits > 128 {
        return false;
    }
    if ty.signed {
        let shift = u32::from(ty.bits - 1);
        let min = -(1i128 << shift);
        let max = (1i128 << shift) - 1;
        value >= min && value <= max
    } else if ty.bits == 128 {
        value >= 0
    } else {
        value >= 0 && (value as u128) < (1u128 << u32::from(ty.bits))
    }
}

fn structured_envelope(
    expected: &BodyType,
    schema: &str,
    value: JsonValue,
    value_digest: &str,
) -> Result<JsonValue, StructuredFail> {
    let mut envelope = Map::new();
    envelope.insert(
        "interface_identity".to_owned(),
        JsonValue::String(expected.canonical_identity()),
    );
    envelope.insert(
        "schema_version".to_owned(),
        JsonValue::String(schema.to_owned()),
    );
    envelope.insert(
        "value_digest".to_owned(),
        JsonValue::String(value_digest.to_owned()),
    );
    envelope.insert("value".to_owned(), value);
    Ok(JsonValue::Object(envelope))
}

/// Encode a contract-shaped external document.  Contract schemas carry their
/// own `schema_version` field at the document boundary; the nominal MNCS
/// interface remains the expected source type at the call site.  The generic
/// envelope remains available for callers that need an explicit interface
/// identity and value digest in the bytes themselves.
fn structured_external_document(
    schema: &str,
    value: JsonValue,
) -> Result<JsonValue, StructuredFail> {
    let JsonValue::Object(mut fields) = value else {
        return Err(StructuredFail::invalid(
            "schema-versioned structured documents require a typed record root",
        ));
    };
    fields.insert(
        "schema_version".to_owned(),
        JsonValue::String(schema.to_owned()),
    );
    Ok(JsonValue::Object(fields))
}

fn find_record<'a>(
    program: &'a Program,
    identity: &SemanticId,
) -> Result<&'a RecordType, StructuredFail> {
    program
        .record_types
        .iter()
        .find(|record| record.identity == *identity)
        .ok_or_else(|| {
            StructuredFail::invalid(format!("record identity {} is not linked", identity.0))
        })
}

fn find_finite<'a>(
    program: &'a Program,
    identity: &SemanticId,
) -> Result<&'a FiniteType, StructuredFail> {
    program
        .finite_types
        .iter()
        .find(|finite| finite.identity == *identity)
        .ok_or_else(|| {
            StructuredFail::invalid(format!("finite identity {} is not linked", identity.0))
        })
}

fn atomic_write(path: &ArtifactPath, bytes: &[u8]) -> Result<(), StructuredFail> {
    let parent = path
        .target
        .parent()
        .ok_or_else(|| StructuredFail::runtime("structured artifact has no parent"))?;
    let pid = std::process::id();
    let mut temporary = None;
    for index in 0..64u32 {
        let candidate = parent.join(format!(".mncs-structured-{pid}-{index}"));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                file.write_all(bytes).map_err(|error| {
                    StructuredFail::runtime(format!(
                        "structured_write temporary artifact {:?} failed: {error}",
                        path.display
                    ))
                })?;
                file.sync_all().map_err(|error| {
                    StructuredFail::runtime(format!(
                        "structured_write temporary artifact {:?} could not be synchronized: {error}",
                        path.display
                    ))
                })?;
                temporary = Some(candidate);
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(StructuredFail::runtime(format!(
                    "structured_write temporary artifact {:?} could not be created: {error}",
                    path.display
                )))
            }
        }
    }
    let temporary = temporary.ok_or_else(|| {
        StructuredFail::runtime("structured_write exhausted its bounded temporary-name space")
    })?;
    if let Err(error) = std::fs::rename(&temporary, &path.target) {
        let _ = std::fs::remove_file(&temporary);
        return Err(StructuredFail::runtime(format!(
            "structured_write atomic replace of {:?} failed: {error}",
            path.display
        )));
    }
    std::fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            StructuredFail::runtime(format!(
                "structured_write publication barrier for {:?} failed: {error}",
                path.display
            ))
        })?;
    std::fs::File::open(&path.root)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            StructuredFail::runtime(format!(
                "structured_write root barrier for {:?} failed: {error}",
                path.display
            ))
        })?;
    Ok(())
}

#[cfg(unix)]
fn open_nofollow(path: &Path, write: bool, append: bool) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut options = std::fs::OpenOptions::new();
    if append {
        options.append(true);
    } else if write {
        options.write(true);
    } else {
        options.read(true);
    }
    options.custom_flags(libc::O_NOFOLLOW).open(path)
}

#[cfg(not(unix))]
fn open_nofollow(path: &Path, write: bool, append: bool) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    if append {
        options.append(true);
    } else if write {
        options.write(true);
    } else {
        options.read(true);
    }
    options.open(path)
}

#[cfg(unix)]
fn is_symlink_refusal(error: &std::io::Error) -> bool {
    error.raw_os_error() == Some(libc::ELOOP)
}

#[cfg(not(unix))]
fn is_symlink_refusal(_error: &std::io::Error) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{record_type_id, IntegerType, RecordField, RecordType};

    fn program() -> (Program, BodyType) {
        let identity = record_type_id(
            "fixture",
            "Artifact",
            &[("count", "u64"), ("name", "[byte; up_to 64]")],
        );
        let record = RecordType {
            identity: identity.clone(),
            name: "Artifact".to_owned(),
            fields: vec![
                RecordField {
                    name: "count".to_owned(),
                    field_type: "u64".to_owned(),
                },
                RecordField {
                    name: "name".to_owned(),
                    field_type: "[byte; up_to 64]".to_owned(),
                },
            ],
        };
        (
            Program {
                schema_version: crate::SUPPORTED_SCHEMA_VERSION.to_owned(),
                module: "fixture".to_owned(),
                dependencies: Vec::new(),
                finite_types: Vec::new(),
                record_types: vec![record],
                assumptions: Vec::new(),
                binding_table: None,
                functions: Vec::new(),
                generic_specializations: Vec::new(),
            },
            BodyType::Record {
                identity,
                name: "Artifact".to_owned(),
            },
        )
    }

    fn value(identity: SemanticId) -> ExecutionValue {
        ExecutionValue::Record {
            type_identity: identity,
            name: "Artifact".to_owned(),
            fields: vec![
                (
                    "count".to_owned(),
                    ExecutionValue::Integer {
                        value: 7,
                        ty: IntegerType {
                            bits: 64,
                            signed: false,
                        },
                    },
                ),
                (
                    "name".to_owned(),
                    ExecutionValue::Sequence {
                        values: b"native"
                            .iter()
                            .map(|byte| ExecutionValue::Byte {
                                value: i128::from(*byte),
                            })
                            .collect::<Vec<_>>()
                            .into(),
                    },
                ),
            ]
            .into(),
        }
    }

    #[test]
    fn round_trip_is_nominal_and_deterministic() {
        let (program, expected) = program();
        let root = tempfile_path("structured-round-trip");
        std::fs::create_dir_all(&root).expect("root");
        let grant = HostGrant {
            capability: "artifact".to_owned(),
            locator: root.to_string_lossy().into_owned(),
            bytes: Vec::new(),
        };
        let original = value(match &expected {
            BodyType::Record { identity, .. } => identity.clone(),
            _ => unreachable!(),
        });
        let (written, _) = structured_write_value(
            &program,
            &expected,
            &original,
            &grant,
            b"artifact.json",
            STRUCTURED_ARTIFACT_SCHEMA_VERSION.as_bytes(),
            true,
        )
        .expect("write");
        assert!(matches!(written, ExecutionValue::Integer { .. }));
        let (decoded, _) = structured_read_value(
            &program,
            &expected,
            &grant,
            b"artifact.json",
            STRUCTURED_ARTIFACT_SCHEMA_VERSION.as_bytes(),
        )
        .expect("read");
        assert_eq!(decoded, original);
        let bytes = std::fs::read(root.join("artifact.json")).expect("artifact");
        assert_eq!(
            bytes,
            std::fs::read(root.join("artifact.json")).expect("stable")
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn schema_identity_and_traversal_fail_closed() {
        let (program, expected) = program();
        let root = tempfile_path("structured-refuse");
        std::fs::create_dir_all(&root).expect("root");
        let grant = HostGrant {
            capability: "artifact".to_owned(),
            locator: root.to_string_lossy().into_owned(),
            bytes: Vec::new(),
        };
        let original = value(match &expected {
            BodyType::Record { identity, .. } => identity.clone(),
            _ => unreachable!(),
        });
        let error = structured_write_value(
            &program,
            &expected,
            &original,
            &grant,
            b"../escape.json",
            STRUCTURED_ARTIFACT_SCHEMA_VERSION.as_bytes(),
            true,
        )
        .expect_err("traversal must fail");
        assert!(matches!(error, StructuredFail::InvalidRequest(_)));
        structured_write_value(
            &program,
            &expected,
            &original,
            &grant,
            b"artifact.json",
            STRUCTURED_ARTIFACT_SCHEMA_VERSION.as_bytes(),
            true,
        )
        .expect("write fixture artifact");
        let error =
            structured_read_value(&program, &expected, &grant, b"artifact.json", b"wrong/1")
                .expect_err("schema mismatch must fail closed");
        assert!(matches!(error, StructuredFail::InvalidRequest(_)));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn retained_value_identity_rejects_tampered_content() {
        let (program, expected) = program();
        let root = tempfile_path("structured-tamper");
        std::fs::create_dir_all(&root).expect("root");
        let grant = HostGrant {
            capability: "artifact".to_owned(),
            locator: root.to_string_lossy().into_owned(),
            bytes: Vec::new(),
        };
        let original = value(match &expected {
            BodyType::Record { identity, .. } => identity.clone(),
            _ => unreachable!(),
        });
        structured_write_value(
            &program,
            &expected,
            &original,
            &grant,
            b"artifact.json",
            STRUCTURED_ARTIFACT_SCHEMA_VERSION.as_bytes(),
            true,
        )
        .expect("write fixture artifact");
        let path = root.join("artifact.json");
        let mut document: JsonValue =
            serde_json::from_slice(&std::fs::read(&path).expect("read")).expect("parse artifact");
        document["value"]["count"] = JsonValue::Number(Number::from(8));
        std::fs::write(
            &path,
            serde_json::to_vec(&document).expect("tampered artifact"),
        )
        .expect("write tampered artifact");
        let error = structured_read_value(
            &program,
            &expected,
            &grant,
            b"artifact.json",
            STRUCTURED_ARTIFACT_SCHEMA_VERSION.as_bytes(),
        )
        .expect_err("tampered value must fail closed");
        assert!(matches!(error, StructuredFail::InvalidRequest(_)));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn schema_versioned_external_document_is_type_directed() {
        let (program, expected) = program();
        let root = tempfile_path("structured-external");
        std::fs::create_dir_all(&root).expect("root");
        let grant = HostGrant {
            capability: "artifact".to_owned(),
            locator: root.to_string_lossy().into_owned(),
            bytes: Vec::new(),
        };
        let original = value(match &expected {
            BodyType::Record { identity, .. } => identity.clone(),
            _ => unreachable!(),
        });
        structured_write_value(
            &program,
            &expected,
            &original,
            &grant,
            b"artifact.json",
            b"fixture/1",
            true,
        )
        .expect("write external document");
        let bytes = std::fs::read(root.join("artifact.json")).expect("artifact");
        let document: JsonValue = serde_json::from_slice(&bytes).expect("external JSON");
        assert_eq!(
            document["schema_version"],
            JsonValue::String("fixture/1".to_owned())
        );
        assert!(document.get("interface_identity").is_none());
        assert!(document.get("value_digest").is_none());
        let (decoded, _) =
            structured_read_value(&program, &expected, &grant, b"artifact.json", b"fixture/1")
                .expect("read external document");
        assert_eq!(decoded, original);
        let _ = std::fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_intermediate_component_is_refused() {
        use std::os::unix::fs::symlink;

        let (program, expected) = program();
        let root = tempfile_path("structured-intermediate-symlink");
        let real = root.join("real");
        std::fs::create_dir_all(&real).expect("root");
        symlink("real", root.join("alias")).expect("alias");
        let grant = HostGrant {
            capability: "artifact".to_owned(),
            locator: root.to_string_lossy().into_owned(),
            bytes: Vec::new(),
        };
        let original = value(match &expected {
            BodyType::Record { identity, .. } => identity.clone(),
            _ => unreachable!(),
        });
        let error = structured_write_value(
            &program,
            &expected,
            &original,
            &grant,
            b"alias/artifact.json",
            STRUCTURED_ARTIFACT_SCHEMA_VERSION.as_bytes(),
            true,
        )
        .expect_err("intermediate symlink must fail closed");
        assert!(matches!(error, StructuredFail::InvalidRequest(_)));
        assert!(!real.join("artifact.json").exists());
        let _ = std::fs::remove_dir_all(root);
    }

    fn tempfile_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "mncs-structured-test-{}-{}",
            std::process::id(),
            label
        ))
    }
}
