//! Residuals: the part of a model value that the ODF construct written for it cannot hold.
//!
//! The writer maps a value (paragraph properties, a table, a list definition) to ODF, reads
//! that ODF back with the reader's own function, and compares the two through their serde
//! form. Every place they differ is recorded as a JSON pointer and the model's value there;
//! the reader applies the corrections after its native mapping. So `model -> odt -> model` is
//! exact, the native mapping does all it can, and what it cannot do is listed (the writer
//! reports each pointer as a value other ODF readers will not see).

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{Map, Value};

/// Corrections: JSON pointers into a value's serde form, each with the value it must have
/// (`None`: absent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Residual(Vec<(String, Option<Value>)>);

fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

fn unescape(segment: &str) -> String {
    segment.replace("~1", "/").replace("~0", "~")
}

fn without(mut value: Value, skip: &[&str]) -> Value {
    if let Value::Object(map) = &mut value {
        for key in skip {
            map.remove(*key);
        }
    }
    value
}

fn diff(path: &str, model: Option<&Value>, native: Option<&Value>, out: &mut Residual) {
    match (model, native) {
        (Some(Value::Object(m)), Some(Value::Object(n))) => {
            let mut keys: Vec<&String> = m.keys().chain(n.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                diff(
                    &format!("{path}/{}", escape(key)),
                    m.get(key),
                    n.get(key),
                    out,
                );
            }
        }
        (Some(Value::Array(m)), Some(Value::Array(n))) if m.len() == n.len() => {
            for (i, (mv, nv)) in m.iter().zip(n).enumerate() {
                diff(&format!("{path}/{i}"), Some(mv), Some(nv), out);
            }
        }
        (m, n) if m == n => {}
        (m, _) => out.0.push((path.to_owned(), m.cloned())),
    }
}

impl Residual {
    /// No corrections.
    #[must_use]
    pub const fn none() -> Self {
        Self(Vec::new())
    }

    /// What `native` lacks to be `model`, ignoring the top-level fields in `skip` (carried by
    /// the document structure, not by this construct).
    ///
    /// # Panics
    /// Never: model values always serialise.
    #[must_use]
    pub fn between<T: Serialize>(model: &T, native: &T, skip: &[&str]) -> Self {
        let m = without(serde_json::to_value(model).expect("model serialises"), skip);
        let n = without(
            serde_json::to_value(native).expect("model serialises"),
            skip,
        );
        let mut out = Self::none();
        diff("", Some(&m), Some(&n), &mut out);
        out
    }

    /// Whether there is nothing to correct.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The corrected value.
    pub fn apply<T: Serialize + DeserializeOwned>(&self, native: &T) -> Result<T, String> {
        let mut value = serde_json::to_value(native).map_err(|e| e.to_string())?;
        for (pointer, replacement) in &self.0 {
            set(&mut value, pointer, replacement.clone())?;
        }
        serde_json::from_value(value).map_err(|e| e.to_string())
    }

    /// Each corrected place, with array indices written `*`: what the writer reports.
    pub fn places(&self) -> impl Iterator<Item = String> + '_ {
        self.0.iter().map(|(pointer, _)| {
            pointer
                .split('/')
                .map(|s| if s.parse::<usize>().is_ok() { "*" } else { s })
                .collect::<Vec<_>>()
                .join("/")
        })
    }

    /// As JSON text, for the sidecar.
    ///
    /// # Panics
    /// Never: JSON values always serialise.
    #[must_use]
    pub fn to_json(&self) -> String {
        let map: Map<String, Value> = self
            .0
            .iter()
            .map(|(p, v)| (p.clone(), v.clone().unwrap_or(Value::Null)))
            .collect();
        serde_json::to_string(&Value::Object(map)).expect("JSON serialises")
    }

    /// From the sidecar's JSON text.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let Value::Object(map) = serde_json::from_str(text).map_err(|e| e.to_string())? else {
            return Err("a residual is a JSON object".to_owned());
        };
        Ok(Self(
            map.into_iter()
                .map(|(p, v)| (p, (!v.is_null()).then_some(v)))
                .collect(),
        ))
    }
}

fn set(root: &mut Value, pointer: &str, replacement: Option<Value>) -> Result<(), String> {
    let segments: Vec<String> = pointer.split('/').skip(1).map(unescape).collect();
    let Some((last, parents)) = segments.split_last() else {
        return match replacement {
            Some(v) => {
                *root = v;
                Ok(())
            }
            None => Err("cannot remove the whole value".to_owned()),
        };
    };
    let mut at = root;
    for segment in parents {
        at = match at {
            Value::Object(map) => map
                .entry(segment.clone())
                .or_insert_with(|| Value::Object(Map::new())),
            Value::Array(items) => segment
                .parse::<usize>()
                .ok()
                .and_then(|i| items.get_mut(i))
                .ok_or_else(|| format!("no element {segment} in {pointer}"))?,
            _ => return Err(format!("{pointer} goes through a scalar")),
        };
    }
    match (at, replacement) {
        (Value::Object(map), Some(v)) => {
            map.insert(last.clone(), v);
        }
        (Value::Object(map), None) => {
            map.remove(last);
        }
        (Value::Array(items), Some(v)) => {
            let slot = last
                .parse::<usize>()
                .ok()
                .and_then(|i| items.get_mut(i))
                .ok_or_else(|| format!("no element {last} in {pointer}"))?;
            *slot = v;
        }
        _ => return Err(format!("{pointer} cannot be set")),
    }
    Ok(())
}

#[cfg(test)]
#[path = "residual_tests.rs"]
mod tests;
