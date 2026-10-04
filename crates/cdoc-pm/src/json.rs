//! The ProseMirror JSON document shape, and helpers for moving model values in and out of
//! attribute maps. A node's attributes are a flat JSON object; the model's serde output for a
//! properties struct is exactly such an object, so encoding is `to_value` and decoding is
//! `from_value` after dropping the `null`s ProseMirror fills in for unset attributes.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// A ProseMirror node as serialised by `node.toJSON()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmNode {
    /// The node type name.
    #[serde(rename = "type")]
    pub kind: String,
    /// Attributes.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub attrs: Map<String, Value>,
    /// Child nodes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<PmNode>,
    /// Marks on an inline node.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<PmMark>,
    /// The text of a `text` node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

impl PmNode {
    /// A node with no attributes, content or marks.
    #[must_use]
    pub fn new(kind: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            attrs: Map::new(),
            content: Vec::new(),
            marks: Vec::new(),
            text: None,
        }
    }

    /// A node with attributes and content.
    #[must_use]
    pub fn with(kind: &str, attrs: Map<String, Value>, content: Vec<PmNode>) -> Self {
        Self {
            kind: kind.to_owned(),
            attrs,
            content,
            marks: Vec::new(),
            text: None,
        }
    }
}

/// A ProseMirror mark.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PmMark {
    /// The mark type name.
    #[serde(rename = "type")]
    pub kind: String,
    /// Attributes.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub attrs: Map<String, Value>,
}

impl PmMark {
    /// A mark with attributes.
    #[must_use]
    pub fn new(kind: &str, attrs: Map<String, Value>) -> Self {
        Self {
            kind: kind.to_owned(),
            attrs,
        }
    }
}

/// Why a ProseMirror document could not be read as a model document. `path` is a slash-separated
/// trail of node types and child indices, e.g. `doc/section[0]/paragraph[2]/text[1]`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{path}: {message}")]
pub struct PmError {
    /// Where.
    pub path: String,
    /// What.
    pub message: String,
}

impl PmError {
    /// An error at a path.
    #[must_use]
    pub fn new(path: &str, message: impl Into<String>) -> Self {
        Self {
            path: path.to_owned(),
            message: message.into(),
        }
    }
}

/// A model value as an attribute map. Panics only if `value` is not a struct, which the
/// callers (all structs) never pass.
#[must_use]
pub fn attrs_from<T: Serialize>(value: &T) -> Map<String, Value> {
    match serde_json::to_value(value) {
        Ok(Value::Object(map)) => map,
        other => unreachable!("properties always serialise to an object: {other:?}"),
    }
}

/// The attribute map read back as a model value, ignoring `null` attributes.
pub fn attrs_into<T: DeserializeOwned>(
    attrs: &Map<String, Value>,
    path: &str,
) -> Result<T, PmError> {
    let stripped: Map<String, Value> = attrs
        .iter()
        .filter(|(_, v)| !v.is_null())
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    serde_json::from_value(Value::Object(stripped))
        .map_err(|e| PmError::new(path, format!("bad attributes: {e}")))
}

/// One attribute read back as a model value; a missing or `null` attribute is an error.
pub fn attr<T: DeserializeOwned>(
    attrs: &Map<String, Value>,
    name: &str,
    path: &str,
) -> Result<T, PmError> {
    match attrs.get(name) {
        None | Some(Value::Null) => Err(PmError::new(path, format!("missing attribute {name:?}"))),
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|e| PmError::new(path, format!("bad attribute {name:?}: {e}"))),
    }
}

/// One optional attribute: `None` when missing or `null`.
pub fn attr_opt<T: DeserializeOwned>(
    attrs: &Map<String, Value>,
    name: &str,
    path: &str,
) -> Result<Option<T>, PmError> {
    match attrs.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => serde_json::from_value(v.clone())
            .map(Some)
            .map_err(|e| PmError::new(path, format!("bad attribute {name:?}: {e}"))),
    }
}

/// Insert a serialisable value under `name`, skipping `None`.
///
/// # Panics
/// Never: every model value serialises to JSON.
pub fn put<T: Serialize>(attrs: &mut Map<String, Value>, name: &str, value: &Option<T>) {
    if let Some(v) = value {
        attrs.insert(
            name.to_owned(),
            serde_json::to_value(v).expect("model values serialise"),
        );
    }
}

/// The path of child `index` of type `kind` under `parent`.
#[must_use]
pub fn child_path(parent: &str, kind: &str, index: usize) -> String {
    format!("{parent}/{kind}[{index}]")
}
