//! Blocks: paragraphs, and the sentinel paragraph OOXML needs after a table.

use super::Writer;
use crate::error::DocxError;
use crate::props::write_ppr;
use crate::seal::is_sealed;
use crate::xml::{Child, Node};
use cdoc_model::{Block, Paragraph};

/// Put a run after a paragraph's properties.
pub(super) fn insert_lead(paragraph: &mut Node, run: Node) {
    let at = usize::from(
        matches!(paragraph.children.first(), Some(Child::Node(n)) if n.name == "w:pPr"),
    );
    paragraph.children.insert(at, Child::Node(run));
}

impl Writer<'_> {
    /// A container's blocks as XML. `sect`, when given, is the `w:sectPr` that ends the
    /// container; it rides in the last paragraph's properties.
    pub(super) fn blocks(
        &mut self,
        blocks: &[Block],
        sect: Option<Node>,
    ) -> Result<Vec<Node>, DocxError> {
        let sealed = is_sealed(blocks);
        let mut sect = sect;
        let last = blocks.len().saturating_sub(1);
        let mut out = Vec::new();
        for (i, block) in blocks.iter().enumerate() {
            match block {
                Block::Paragraph(p) => {
                    let ends = if i == last && !sealed {
                        sect.take()
                    } else {
                        None
                    };
                    out.push(self.paragraph(p, ends)?);
                }
                Block::Table(t) => out.push(self.table(t)?),
            }
        }
        if sealed || blocks.is_empty() {
            let mut sentinel = Node::new("w:p");
            if let Some(s) = sect.take() {
                sentinel = sentinel.child(Node::new("w:pPr").child(s));
            }
            out.push(sentinel);
        }
        Ok(out)
    }

    pub(super) fn paragraph(
        &mut self,
        p: &Paragraph,
        sect: Option<Node>,
    ) -> Result<Node, DocxError> {
        let mut ppr = write_ppr(&p.properties, &mut self.log);
        if let Some(s) = sect {
            ppr = ppr.child(s);
        }
        let mut node = Node::new("w:p");
        if !ppr.children.is_empty() {
            node = node.child(ppr);
        }
        Ok(node.children(self.inlines(&p.inlines)?))
    }
}
