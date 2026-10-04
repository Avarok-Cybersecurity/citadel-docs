//! Lossless mapping between the Citadel Docs model and ProseMirror JSON, and the ProseMirror
//! schema that mapping implies.
//!
//! `to_pm` and `from_pm` are inverse on every model document: `from_pm(&to_pm(&d)) == d`. The
//! schema ([`schema`]) is generated from the same names and the same model types the mapping
//! uses, so the TipTap schema in the UI is checked against this crate's output rather than
//! maintained by hand; see `docs/tiptap-schema.md`.

#![forbid(unsafe_code)]

mod decode;
mod encode;
pub mod json;
pub mod names;
pub mod schema;

pub use json::{PmError, PmMark, PmNode};
pub use schema::{MarkSpec, NodeSpec, SchemaSpec};

use cdoc_model::Document;

/// The document as a ProseMirror `doc` node.
#[must_use]
pub fn to_pm(doc: &Document) -> PmNode {
    encode::encode_document(doc)
}

/// The document from a ProseMirror `doc` node.
pub fn from_pm(node: &PmNode) -> Result<Document, PmError> {
    decode::decode_document(node)
}

/// The document as ProseMirror JSON text.
///
/// # Panics
/// Never: every model value serialises to JSON.
#[must_use]
pub fn to_pm_json(doc: &Document) -> String {
    serde_json::to_string(&to_pm(doc)).expect("ProseMirror nodes serialise")
}

/// The document from ProseMirror JSON text.
pub fn from_pm_json(text: &str) -> Result<Document, PmError> {
    let node: PmNode = serde_json::from_str(text)
        .map_err(|e| PmError::new("", format!("not a ProseMirror node: {e}")))?;
    from_pm(&node)
}
