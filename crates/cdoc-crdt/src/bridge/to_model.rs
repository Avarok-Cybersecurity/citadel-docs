//! CRDT to model: one pass over the sequence in document order.

use super::build::Builder;
use super::names;
use super::slice::{walk, Piece};
use super::BridgeError;
use crate::attrs::{AttrKey, List};
use crate::codec::from_bytes;
use crate::doc::Doc;
use cdoc_model::{Document, Numbering, StyleSheet};
use serde::de::DeserializeOwned;

fn attr<T: DeserializeOwned>(doc: &Doc, name: &'static str) -> Result<T, BridgeError> {
    let write = doc
        .attrs
        .get(&AttrKey::Name(name.to_owned()))
        .ok_or(BridgeError::MissingAttr(name))?;
    Ok(from_bytes(&write.value)?)
}

/// The document this replica shows.
pub fn to_model(doc: &Doc) -> Result<Document, BridgeError> {
    let mut builder = Builder::new();
    walk(doc, None, &mut |piece| match piece {
        Piece::Inline(inline) => builder.inline(inline),
        Piece::Block(atom) => builder.atom(atom),
    })?;
    let (sections, footnotes) = builder.finish()?;
    Ok(Document {
        meta: attr(doc, names::META)?,
        settings: attr(doc, names::SETTINGS)?,
        styles: StyleSheet {
            defaults: attr(doc, names::STYLES)?,
            styles: doc.entry_list(List::Styles)?,
        },
        numbering: Numbering {
            definitions: doc.entry_list(List::NumberingDefinitions)?,
            instances: doc.entry_list(List::NumberingInstances)?,
        },
        sections,
        footnotes,
        comments: doc.entry_list(List::Comments)?,
    })
}
