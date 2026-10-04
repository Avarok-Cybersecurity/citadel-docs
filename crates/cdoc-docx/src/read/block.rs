//! Block containers: flattening content controls, and removing the writer's sentinel.

use super::Reader;
use crate::error::DocxError;
use crate::seal::unseal;
use crate::xml::Node;
use cdoc_model::{Block, Paragraph, ParagraphProperties};

/// A paragraph with no properties and no content.
pub(super) fn bare_paragraph() -> Block {
    Block::Paragraph(Paragraph {
        properties: ParagraphProperties::none(),
        inlines: Vec::new(),
    })
}

impl Reader<'_> {
    /// The block-level children of `parent` (paragraphs, tables, section properties), looking
    /// through block content controls.
    pub(super) fn block_nodes<'n>(&mut self, parent: &'n Node, out: &mut Vec<&'n Node>) {
        for child in parent.elems() {
            match child.name.as_str() {
                "w:p" | "w:tbl" | "w:sectPr" => out.push(child),
                "w:sdt" => {
                    self.log.lose("block-level content control unwrapped");
                    if let Some(content) = child.find("w:sdtContent") {
                        self.block_nodes(content, out);
                    }
                }
                "w:tcPr" | "w:sdtPr" | "w:sdtEndPr" | "w:proofErr" => {}
                "w:bookmarkStart" | "w:bookmarkEnd" => self.log.lose(&child.name),
                other => self.log.unknown(&format!("{}/{other}", parent.name)),
            }
        }
    }

    /// The blocks inside a header, footer, footnote or cell. Never empty: a container with no
    /// content reads as one empty paragraph.
    pub(super) fn container_blocks(&mut self, parent: &Node) -> Result<Vec<Block>, DocxError> {
        let mut nodes = Vec::new();
        self.block_nodes(parent, &mut nodes);
        let mut blocks = Vec::new();
        for node in nodes {
            match node.name.as_str() {
                "w:p" => blocks.push(Block::Paragraph(self.paragraph(node)?.0)),
                "w:tbl" => blocks.push(Block::Table(self.table(node)?)),
                _ => self.log.lose("w:sectPr outside the body"),
            }
        }
        unseal(&mut blocks);
        if blocks.is_empty() {
            blocks.push(bare_paragraph());
        }
        Ok(blocks)
    }
}
