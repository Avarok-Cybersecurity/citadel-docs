//! Footnotes and comments.

use super::block::insert_lead;
use super::Writer;
use crate::error::DocxError;
use crate::ns::declare;
use crate::settings::SEPARATOR_IDS;
use crate::xml::Node;
use cdoc_model::Block;
use cdoc_model::Document;

/// The Word 2013 paragraph id of the `index`th comment's last paragraph.
pub(crate) fn para_id(index: usize) -> String {
    format!("{:08X}", 0x1000_0000 + index)
}

impl Writer<'_> {
    pub(super) fn footnotes(&mut self, doc: &Document) -> Result<Node, DocxError> {
        let separator = |id: i32, kind: &str, mark: &str| {
            Node::new("w:footnote")
                .attr("w:type", kind)
                .num("w:id", id)
                .child(Node::new("w:p").child(Node::new("w:r").child(Node::new(mark))))
        };
        let mut root = Node::new("w:footnotes")
            .child(separator(SEPARATOR_IDS[0], "separator", "w:separator"))
            .child(separator(
                SEPARATOR_IDS[1],
                "continuationSeparator",
                "w:continuationSeparator",
            ));
        for note in &doc.footnotes {
            let mut nodes = self.blocks(&note.blocks, None)?;
            if matches!(note.blocks.first(), Some(Block::Paragraph(_))) {
                if let Some(first) = nodes.first_mut() {
                    insert_lead(first, Node::new("w:r").child(Node::new("w:footnoteRef")));
                }
            }
            root = root.child(
                Node::new("w:footnote")
                    .num("w:id", note.id.0)
                    .children(nodes),
            );
        }
        Ok(declare(root))
    }

    pub(super) fn comments(&mut self, doc: &Document) -> Node {
        let mut root = Node::new("w:comments");
        for (index, comment) in doc.comments.iter().enumerate() {
            let n = self.ids.comments.number(comment.id.as_str());
            let lines: Vec<&str> = comment.text.split('\n').collect();
            let mut node = Node::new("w:comment")
                .num("w:id", n)
                .attr("w:author", &comment.author)
                .attr("w:date", &comment.date);
            for (i, line) in lines.iter().enumerate() {
                let mut p = Node::new("w:p");
                if i + 1 == lines.len() {
                    p = p.attr("w14:paraId", &para_id(index));
                }
                if !line.is_empty() {
                    let t = Node::new("w:t").attr("xml:space", "preserve").text(line);
                    p = p.child(Node::new("w:r").child(t));
                }
                node = node.child(p);
            }
            root = root.child(node);
        }
        declare(root)
    }

    pub(super) fn comments_extended(&mut self, doc: &Document) -> Node {
        let mut root = Node::new("w15:commentsEx");
        for (index, comment) in doc.comments.iter().enumerate() {
            let mut ex = Node::new("w15:commentEx").attr("w15:paraId", &para_id(index));
            if let Some(parent) = &comment.parent {
                match doc.comments.iter().position(|c| &c.id == parent) {
                    Some(at) => ex = ex.attr("w15:paraIdParent", &para_id(at)),
                    None => self
                        .log
                        .lose("a comment replies to a comment that does not exist"),
                }
            }
            root = root.child(ex.attr("w15:done", if comment.resolved { "1" } else { "0" }));
        }
        declare(root)
    }
}
