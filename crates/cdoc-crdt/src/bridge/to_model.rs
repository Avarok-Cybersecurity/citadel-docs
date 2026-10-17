//! CRDT to model: one pass over the sequence in document order.

use super::build::Builder;
use super::names;
use super::resolve::Resolver;
use super::{Atom, BridgeError};
use crate::attrs::AttrKey;
use crate::codec::from_bytes;
use crate::doc::Doc;
use crate::item::{byte_prefix, AtomKind, Content};
use cdoc_model::{Document, Inline, InlineContent, TextRun};
use serde::de::DeserializeOwned;

fn attr<T: DeserializeOwned>(doc: &Doc, name: &'static str) -> Result<T, BridgeError> {
    let write = doc
        .attrs
        .get(&AttrKey::Name(name.to_owned()))
        .ok_or(BridgeError::MissingAttr(name))?;
    Ok(from_bytes(&write.value)?)
}

/// The atom an item holds, with its block properties as last written.
fn atom(doc: &Doc, id: crate::id::Id, index: u32, kind: AtomKind) -> Result<Atom, BridgeError> {
    let written = kind
        .is_structural()
        .then(|| doc.attrs.get(&AttrKey::Block(id)))
        .flatten();
    let payload = written.map_or(doc.atoms[index as usize].as_slice(), |w| &w.value);
    Ok(Atom::decode(kind, payload)?)
}

/// The document this replica shows.
pub fn to_model(doc: &Doc) -> Result<Document, BridgeError> {
    let mut positions = vec![0u64; doc.seq.item_count()];
    let mut pos = 0u64;
    for idx in doc.seq.iter() {
        positions[idx as usize] = pos;
        pos += u64::from(doc.seq.item(idx).len);
    }
    let mut marks = Resolver::new(doc, &positions);
    let mut builder = Builder::new();
    for idx in doc.seq.iter() {
        let item = doc.seq.item(idx);
        let start = positions[idx as usize];
        match item.content {
            Content::Gone => {}
            Content::Text { start: at, bytes } => {
                let mut rest = &doc.text[at as usize..(at + bytes) as usize];
                let end = start + u64::from(item.len);
                let mut u = start;
                while u < end {
                    marks.seek(u);
                    let until = marks.next_change().min(end);
                    let chars = usize::try_from(until - u).unwrap_or(usize::MAX);
                    let cut = byte_prefix(rest, chars);
                    let (properties, inline_marks) = marks.current()?;
                    builder.inline(Inline {
                        content: InlineContent::Text(TextRun {
                            text: rest[..cut].to_owned(),
                            properties,
                        }),
                        marks: inline_marks,
                    });
                    rest = &rest[cut..];
                    u = until;
                }
            }
            Content::Atom { index, kind } => match atom(doc, item.id, index, kind)? {
                Atom::Inline(content) => {
                    marks.seek(start);
                    let (_, inline_marks) = marks.current()?;
                    builder.inline(Inline {
                        content,
                        marks: inline_marks,
                    });
                }
                structural => builder.atom(structural),
            },
        }
    }
    let (sections, footnotes) = builder.finish()?;
    Ok(Document {
        meta: attr(doc, names::META)?,
        settings: attr(doc, names::SETTINGS)?,
        styles: attr(doc, names::STYLES)?,
        numbering: attr(doc, names::NUMBERING)?,
        sections,
        footnotes,
        comments: attr(doc, names::COMMENTS)?,
    })
}
