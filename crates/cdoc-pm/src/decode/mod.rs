//! ProseMirror JSON → model. Strict: an unknown node or mark type, a missing required
//! attribute, or a node where the schema does not allow one is an error that names its path.
//! Structural rules the model leaves to `Document::validate` (non-empty sections, dangling
//! ids) are not re-checked here, so decoding is lossless on any document the encoder emits.

mod block;
mod inline;

pub(crate) use block::decode_blocks;
pub(crate) use inline::decode_inline;

use crate::json::{attr, attrs_into, child_path, PmError, PmNode};
use crate::names::{attr as attr_name, node};
use cdoc_model::{
    Document, Footnote, HeaderFooter, HeaderFooterKind, HeaderFooterSet, Section, SectionProperties,
};

/// The document from its `doc` node.
pub fn decode_document(root: &PmNode) -> Result<Document, PmError> {
    let path = node::DOC;
    expect_kind(root, node::DOC, path)?;
    let mut sections = Vec::new();
    let mut footnotes: Option<Vec<Footnote>> = None;
    for (i, child) in root.content.iter().enumerate() {
        match child.kind.as_str() {
            node::SECTION => sections.push(decode_section(
                child,
                &child_path(path, node::SECTION, sections.len()),
            )?),
            node::FOOTNOTES if footnotes.is_none() => {
                footnotes = Some(decode_footnotes(
                    child,
                    &child_path(path, node::FOOTNOTES, 0),
                )?);
            }
            node::FOOTNOTES => {
                return Err(PmError::new(path, "more than one footnotes node"));
            }
            other => {
                return Err(PmError::new(
                    &child_path(path, other, i),
                    format!("{other:?} is not allowed in doc"),
                ))
            }
        }
    }
    Ok(Document {
        meta: attr(&root.attrs, attr_name::META, path)?,
        settings: attr(&root.attrs, attr_name::SETTINGS, path)?,
        styles: attr(&root.attrs, attr_name::STYLES, path)?,
        numbering: attr(&root.attrs, attr_name::NUMBERING, path)?,
        sections,
        footnotes: footnotes.unwrap_or_default(),
        comments: attr(&root.attrs, attr_name::COMMENTS, path)?,
    })
}

/// A section: page setup from attributes; header, footer and block children.
pub fn decode_section(node: &PmNode, path: &str) -> Result<Section, PmError> {
    expect_kind(node, node::SECTION, path)?;
    let mut attrs = node.attrs.clone();
    attrs.insert(
        "headers".to_owned(),
        serde_json::to_value(HeaderFooterSet::none()).expect("serialises"),
    );
    attrs.insert(
        "footers".to_owned(),
        serde_json::to_value(HeaderFooterSet::none()).expect("serialises"),
    );
    let mut properties: SectionProperties = attrs_into(&attrs, path)?;
    let mut blocks = Vec::new();
    for child in &node.content {
        match child.kind.as_str() {
            node::HEADER | node::FOOTER => {
                let set = if child.kind == node::HEADER {
                    &mut properties.headers
                } else {
                    &mut properties.footers
                };
                let hf_path = child_path(path, &child.kind, 0);
                let kind: HeaderFooterKind = attr(&child.attrs, attr_name::KIND, &hf_path)?;
                let slot = set.slot(kind);
                if slot.is_some() {
                    return Err(PmError::new(
                        &hf_path,
                        format!("duplicate {} of kind {kind:?}", child.kind),
                    ));
                }
                *slot = Some(HeaderFooter {
                    blocks: decode_blocks(&child.content, &hf_path)?,
                });
            }
            _ => blocks.push(child.clone()),
        }
    }
    Ok(Section {
        properties,
        blocks: decode_blocks(&blocks, path)?,
    })
}

fn decode_footnotes(node: &PmNode, path: &str) -> Result<Vec<Footnote>, PmError> {
    node.content
        .iter()
        .enumerate()
        .map(|(i, child)| {
            let fn_path = child_path(path, node::FOOTNOTE, i);
            expect_kind(child, node::FOOTNOTE, &fn_path)?;
            Ok(Footnote {
                id: attr(&child.attrs, attr_name::ID, &fn_path)?,
                blocks: decode_blocks(&child.content, &fn_path)?,
            })
        })
        .collect()
}

pub(crate) fn expect_kind(node: &PmNode, kind: &str, path: &str) -> Result<(), PmError> {
    if node.kind == kind {
        Ok(())
    } else {
        Err(PmError::new(
            path,
            format!("expected a {kind:?} node, found {:?}", node.kind),
        ))
    }
}
