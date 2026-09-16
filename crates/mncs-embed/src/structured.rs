//! Bounded structured-data transport for native applications.
//!
//! Native modules remain the semantic authority.  This module is deliberately
//! only an external-boundary codec: it validates UTF-8/JSON input, enforces
//! resource bounds, and emits deterministic compact JSON with object keys in
//! lexical order.  It does not assign meaning to application records.

use std::collections::BTreeMap;

use serde_json::{Map, Value};

pub const MAX_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_DEPTH: usize = 64;
pub const MAX_NODES: usize = 32_768;
pub const MAX_COLLECTION_ITEMS: usize = 8_192;
pub const MAX_STRING_BYTES: usize = 1 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredDocument {
    value: Value,
    canonical: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredDataError {
    TooLarge { limit: usize },
    InvalidUtf8,
    InvalidJson(String),
    DepthLimit { limit: usize },
    NodeLimit { limit: usize },
    CollectionLimit { limit: usize },
    StringLimit { limit: usize },
    NonFiniteNumber,
}

impl std::fmt::Display for StructuredDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooLarge { limit } => write!(f, "structured document exceeds {limit} bytes"),
            Self::InvalidUtf8 => write!(f, "structured document is not valid UTF-8"),
            Self::InvalidJson(message) => {
                write!(f, "structured document is not valid JSON: {message}")
            }
            Self::DepthLimit { limit } => {
                write!(f, "structured document exceeds depth limit {limit}")
            }
            Self::NodeLimit { limit } => {
                write!(f, "structured document exceeds node limit {limit}")
            }
            Self::CollectionLimit { limit } => {
                write!(f, "structured collection exceeds item limit {limit}")
            }
            Self::StringLimit { limit } => write!(f, "structured string exceeds {limit} bytes"),
            Self::NonFiniteNumber => write!(f, "structured number is not finite"),
        }
    }
}

impl std::error::Error for StructuredDataError {}

impl StructuredDocument {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, StructuredDataError> {
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err(StructuredDataError::TooLarge {
                limit: MAX_DOCUMENT_BYTES,
            });
        }
        std::str::from_utf8(bytes).map_err(|_| StructuredDataError::InvalidUtf8)?;
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|error| StructuredDataError::InvalidJson(error.to_string()))?;
        let mut nodes = 0;
        let canonical_value = normalize(&value, 0, &mut nodes)?;
        let canonical = serde_json::to_vec(&canonical_value)
            .map_err(|error| StructuredDataError::InvalidJson(error.to_string()))?;
        if canonical.len() > MAX_DOCUMENT_BYTES {
            return Err(StructuredDataError::TooLarge {
                limit: MAX_DOCUMENT_BYTES,
            });
        }
        Ok(Self {
            value: canonical_value,
            canonical,
        })
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical
    }

    pub fn into_value(self) -> Value {
        self.value
    }
}

pub fn canonical_bytes(value: &Value) -> Result<Vec<u8>, StructuredDataError> {
    let mut nodes = 0;
    let normalized = normalize(value, 0, &mut nodes)?;
    let bytes = serde_json::to_vec(&normalized)
        .map_err(|error| StructuredDataError::InvalidJson(error.to_string()))?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return Err(StructuredDataError::TooLarge {
            limit: MAX_DOCUMENT_BYTES,
        });
    }
    Ok(bytes)
}

fn normalize(value: &Value, depth: usize, nodes: &mut usize) -> Result<Value, StructuredDataError> {
    if depth > MAX_DEPTH {
        return Err(StructuredDataError::DepthLimit { limit: MAX_DEPTH });
    }
    *nodes = nodes.saturating_add(1);
    if *nodes > MAX_NODES {
        return Err(StructuredDataError::NodeLimit { limit: MAX_NODES });
    }
    match value {
        Value::Null | Value::Bool(_) => Ok(value.clone()),
        Value::Number(number) => {
            if number.as_f64().is_some_and(|number| !number.is_finite()) {
                return Err(StructuredDataError::NonFiniteNumber);
            }
            Ok(value.clone())
        }
        Value::String(text) => {
            if text.len() > MAX_STRING_BYTES {
                return Err(StructuredDataError::StringLimit {
                    limit: MAX_STRING_BYTES,
                });
            }
            Ok(value.clone())
        }
        Value::Array(items) => {
            if items.len() > MAX_COLLECTION_ITEMS {
                return Err(StructuredDataError::CollectionLimit {
                    limit: MAX_COLLECTION_ITEMS,
                });
            }
            items
                .iter()
                .map(|item| normalize(item, depth + 1, nodes))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array)
        }
        Value::Object(fields) => {
            if fields.len() > MAX_COLLECTION_ITEMS {
                return Err(StructuredDataError::CollectionLimit {
                    limit: MAX_COLLECTION_ITEMS,
                });
            }
            let mut ordered = BTreeMap::new();
            for (key, item) in fields {
                if key.len() > MAX_STRING_BYTES {
                    return Err(StructuredDataError::StringLimit {
                        limit: MAX_STRING_BYTES,
                    });
                }
                ordered.insert(key.clone(), normalize(item, depth + 1, nodes)?);
            }
            Ok(Value::Object(ordered.into_iter().collect::<Map<_, _>>()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_object_order_and_preserves_arrays() {
        let document = StructuredDocument::from_bytes(br#"{"z":1,"a":[true,null]}"#).unwrap();
        assert_eq!(document.canonical_bytes(), br#"{"a":[true,null],"z":1}"#);
    }

    #[test]
    fn rejects_excessive_depth() {
        let input = format!(
            "{}0{}",
            "[".repeat(MAX_DEPTH + 2),
            "]".repeat(MAX_DEPTH + 2)
        );
        assert!(matches!(
            StructuredDocument::from_bytes(input.as_bytes()),
            Err(StructuredDataError::DepthLimit { .. })
        ));
    }
}
