//! Block properties field by field. A block's properties are its structural atom's payload as
//! inserted, overridden per field by attribute writes (`AttrKey::Field`), each its own
//! register: concurrent edits of different fields of one block (alignment here, indent there)
//! both survive. A field path names a field of a struct-valued part (`alignment`, or `0.style`
//! when the payload is a tuple) or a whole leaf part (`1`, or the empty path when the payload
//! is one leaf). An empty value is an absent field.

use crate::codec::raw::{map_entries, seq_items, write_map, write_seq};
use crate::codec::CodecError;
use crate::item::AtomKind;
use std::borrow::Cow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Part {
    /// A struct: merged field by field.
    Struct,
    /// Anything else: one register.
    Leaf,
}

/// How each kind's payload is laid out (`Atom::encode`).
const fn shape(kind: AtomKind) -> &'static [Part] {
    match kind {
        AtomKind::Paragraph | AtomKind::Section => &[Part::Struct],
        AtomKind::Footnote => &[Part::Leaf],
        AtomKind::Table | AtomKind::Cell => &[Part::Struct, Part::Leaf],
        AtomKind::Row | AtomKind::HeaderFooter => &[Part::Leaf, Part::Leaf],
        AtomKind::Inline | AtomKind::End => &[],
    }
}

/// Field paths and encoded values, in encoded order.
type Fields<'a> = Vec<(Cow<'a, str>, &'a [u8])>;

/// Changed fields: path, and the new value (`None`: absent).
pub(crate) type FieldChanges = Vec<(String, Option<Vec<u8>>)>;

fn find<'v>(fields: &[(Cow<'_, str>, &'v [u8])], path: &str) -> Option<&'v [u8]> {
    fields.iter().find(|(p, _)| p == path).map(|&(_, v)| v)
}

fn parts(kind: AtomKind, payload: &[u8]) -> Result<Vec<&[u8]>, CodecError> {
    match shape(kind).len() {
        0 => Ok(Vec::new()),
        1 => Ok(vec![payload]),
        n => seq_items(payload, n),
    }
}

fn leaf_path(multi: bool, i: usize) -> String {
    if multi {
        i.to_string()
    } else {
        String::new()
    }
}

/// Every field present, with its path and encoded value, in encoded order.
fn split(kind: AtomKind, payload: &[u8]) -> Result<Fields<'_>, CodecError> {
    let shape = shape(kind);
    let multi = shape.len() > 1;
    let mut out = Vec::new();
    for (i, (part, bytes)) in shape.iter().zip(parts(kind, payload)?).enumerate() {
        match part {
            Part::Struct => {
                for (name, value) in map_entries(bytes)? {
                    let path = if multi {
                        Cow::Owned(format!("{i}.{name}"))
                    } else {
                        Cow::Borrowed(name)
                    };
                    out.push((path, value));
                }
            }
            Part::Leaf => out.push((Cow::Owned(leaf_path(multi, i)), bytes)),
        }
    }
    Ok(out)
}

/// The fields whose values differ between `old` and `new`, with the new value (`None`: the
/// field is absent from `new`): new and changed fields in `new`'s order, then removed ones.
pub(crate) fn diff(kind: AtomKind, old: &[u8], new: &[u8]) -> Result<FieldChanges, CodecError> {
    let (old, new) = (split(kind, old)?, split(kind, new)?);
    let mut out = Vec::new();
    for (path, value) in &new {
        if find(&old, path) != Some(*value) {
            out.push((path.to_string(), Some(value.to_vec())));
        }
    }
    for (path, _) in &old {
        if find(&new, path).is_none() {
            out.push((path.to_string(), None));
        }
    }
    Ok(out)
}

/// `base` with `overrides` applied: a `Some` value sets the field, `None` removes it.
/// Overrides are in path order, so fields new to `base` are appended in path order.
pub(crate) fn assemble(
    kind: AtomKind,
    base: &[u8],
    overrides: &[(&str, Option<&[u8]>)],
) -> Result<Vec<u8>, CodecError> {
    let shape = shape(kind);
    let multi = shape.len() > 1;
    let base_parts = parts(kind, base)?;
    let mut used = vec![false; overrides.len()];
    let mut out: Vec<Vec<u8>> = Vec::with_capacity(shape.len());
    for (i, (part, bytes)) in shape.iter().zip(base_parts).enumerate() {
        match part {
            Part::Struct => {
                let prefix = if multi {
                    format!("{i}.")
                } else {
                    String::new()
                };
                let mut entries = map_entries(bytes)?;
                for (k, (path, value)) in overrides.iter().enumerate() {
                    let Some(name) = path.strip_prefix(prefix.as_str()) else {
                        continue;
                    };
                    used[k] = true;
                    let at = entries.iter().position(|(n, _)| *n == name);
                    match (at, value) {
                        (Some(at), Some(v)) => entries[at].1 = v,
                        (Some(at), None) => {
                            entries.remove(at);
                        }
                        (None, Some(v)) => entries.push((name, v)),
                        (None, None) => {}
                    }
                }
                out.push(write_map(&entries));
            }
            Part::Leaf => {
                let path = leaf_path(multi, i);
                let mut value = bytes;
                for (k, (p, v)) in overrides.iter().enumerate() {
                    if *p == path {
                        used[k] = true;
                        value = v.ok_or_else(|| absent(&path))?;
                    }
                }
                out.push(value.to_vec());
            }
        }
    }
    if let Some(k) = used.iter().position(|u| !u) {
        return Err(CodecError::Message(format!(
            "{kind:?} properties have no field {:?}",
            overrides[k].0
        )));
    }
    Ok(match out.len() {
        // An end atom: no payload at all.
        0 => Vec::new(),
        1 => out.remove(0),
        _ => write_seq(&out.iter().map(Vec::as_slice).collect::<Vec<_>>()),
    })
}

fn absent(path: &str) -> CodecError {
    CodecError::Message(format!("the property {path:?} cannot be absent"))
}

/// The properties of block `id` as they stand: its atom's payload `base`, overridden by every
/// field written since.
pub(crate) fn properties<'d>(
    doc: &'d crate::doc::Doc,
    id: crate::id::Id,
    kind: AtomKind,
    base: &'d [u8],
) -> Result<Cow<'d, [u8]>, CodecError> {
    let overrides: Vec<(&str, Option<&[u8]>)> = doc
        .attrs
        .fields(id)
        .map(|(path, w)| (path, (!w.value.is_empty()).then_some(w.value.as_slice())))
        .collect();
    if overrides.is_empty() {
        return Ok(Cow::Borrowed(base));
    }
    Ok(Cow::Owned(assemble(kind, base, &overrides)?))
}
