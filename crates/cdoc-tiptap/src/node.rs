//! A read-only view of one ProseMirror JSON node or mark: an object with a string `type`.
//! Everything else about it is optional and read leniently; what does not fit is the caller's
//! to report.

use serde_json::{Map, Value};

/// A node or a mark.
#[derive(Clone, Copy)]
pub(crate) struct Node<'a> {
    pub(crate) kind: &'a str,
    obj: &'a Map<String, Value>,
}

/// What a node's `content` (or `marks`) holds.
pub(crate) enum Children<'a> {
    /// The array, possibly empty; also an absent or `null` member.
    Array(&'a [Value]),
    /// A member that is not an array.
    Malformed,
}

impl<'a> Node<'a> {
    /// The node `value` is, if it is one.
    pub(crate) fn of(value: &'a Value) -> Option<Self> {
        let obj = value.as_object()?;
        let kind = obj.get("type")?.as_str()?;
        Some(Self { kind, obj })
    }

    fn array(&self, key: &str) -> Children<'a> {
        match self.obj.get(key) {
            None | Some(Value::Null) => Children::Array(&[]),
            Some(Value::Array(items)) => Children::Array(items),
            Some(_) => Children::Malformed,
        }
    }

    /// The child nodes.
    pub(crate) fn content(&self) -> Children<'a> {
        self.array("content")
    }

    /// The marks.
    pub(crate) fn marks(&self) -> Children<'a> {
        self.array("marks")
    }

    /// The `text` of a text node.
    pub(crate) fn text(&self) -> Option<&'a str> {
        self.obj.get("text")?.as_str()
    }

    /// An attribute that is present and not `null`.
    pub(crate) fn attr(&self, key: &str) -> Option<&'a Value> {
        self.obj
            .get("attrs")?
            .as_object()?
            .get(key)
            .filter(|v| !v.is_null())
    }

    /// The names of the attributes that are present and not `null`, in the input's order.
    pub(crate) fn attr_names(&self) -> Vec<&'a str> {
        self.obj
            .get("attrs")
            .and_then(Value::as_object)
            .map(|attrs| {
                attrs
                    .iter()
                    .filter(|(_, v)| !v.is_null())
                    .map(|(k, _)| k.as_str())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// An attribute value as a whole number: a JSON integer, or a string of digits as some
/// serialisers write attributes. Never a float: `1.5` is not a whole number.
pub(crate) fn whole(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}
