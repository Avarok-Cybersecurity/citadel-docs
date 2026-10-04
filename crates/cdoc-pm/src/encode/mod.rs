//! Model → ProseMirror JSON. Document-level attributes carry what the editor does not show in
//! the tree (metadata, settings, styles, numbering, comment bodies); everything else is a node.

mod block;
mod inline;

pub(crate) use block::encode_blocks;
pub(crate) use inline::encode_inline;

use crate::json::{attrs_from, PmNode};
use crate::names::{attr, node};
use cdoc_model::{Document, HeaderFooterSet, Section};
use serde_json::{Map, Value};

/// The whole document as a `doc` node.
#[must_use]
pub fn encode_document(doc: &Document) -> PmNode {
    let mut attrs = Map::new();
    attrs.insert(attr::META.to_owned(), to_value(&doc.meta));
    attrs.insert(attr::SETTINGS.to_owned(), to_value(&doc.settings));
    attrs.insert(attr::STYLES.to_owned(), to_value(&doc.styles));
    attrs.insert(attr::NUMBERING.to_owned(), to_value(&doc.numbering));
    attrs.insert(attr::COMMENTS.to_owned(), to_value(&doc.comments));
    let mut content: Vec<PmNode> = doc.sections.iter().map(encode_section).collect();
    if !doc.footnotes.is_empty() {
        let footnotes = doc
            .footnotes
            .iter()
            .map(|f| {
                let mut attrs = Map::new();
                attrs.insert(attr::ID.to_owned(), to_value(&f.id));
                PmNode::with(node::FOOTNOTE, attrs, encode_blocks(&f.blocks))
            })
            .collect();
        content.push(PmNode::with(node::FOOTNOTES, Map::new(), footnotes));
    }
    PmNode::with(node::DOC, attrs, content)
}

/// A section: page setup as attributes, headers and footers as leading children, then blocks.
#[must_use]
pub fn encode_section(section: &Section) -> PmNode {
    let mut attrs = attrs_from(&section.properties);
    attrs.remove("headers");
    attrs.remove("footers");
    let mut content = Vec::new();
    content.extend(encode_header_footers(
        node::HEADER,
        &section.properties.headers,
    ));
    content.extend(encode_header_footers(
        node::FOOTER,
        &section.properties.footers,
    ));
    content.extend(encode_blocks(&section.blocks));
    PmNode::with(node::SECTION, attrs, content)
}

fn encode_header_footers(kind: &str, set: &HeaderFooterSet) -> Vec<PmNode> {
    set.present()
        .into_iter()
        .map(|(which, hf)| {
            let mut attrs = Map::new();
            attrs.insert(attr::KIND.to_owned(), to_value(&which));
            PmNode::with(kind, attrs, encode_blocks(&hf.blocks))
        })
        .collect()
}

pub(crate) fn to_value<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("model values serialise")
}
