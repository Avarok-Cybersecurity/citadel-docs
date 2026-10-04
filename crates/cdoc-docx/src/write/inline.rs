//! Paragraph content. Marks are properties of each inline in the model; in `.docx` they are
//! wrappers and range markers, so adjacent inlines with equal marks share a wrapper:
//! hyperlink outside, tracked change inside, comment ranges between runs.

use super::Writer;
use crate::error::DocxError;
use crate::xml::Node;
use cdoc_model::{CommentId, Inline, Link, Revision, RevisionKind};

impl Writer<'_> {
    pub(super) fn inlines(&mut self, inlines: &[Inline]) -> Result<Vec<Node>, DocxError> {
        let mut out = Vec::new();
        let mut active: Vec<CommentId> = Vec::new();
        for link_group in inlines.chunk_by(|a, b| a.marks.link == b.marks.link) {
            let link = &link_group[0].marks.link;
            let mut inner = Vec::new();
            for group in link_group.chunk_by(|a, b| a.marks.revision == b.marks.revision) {
                let rev = &group[0].marks.revision;
                let deleted = matches!(rev, Some(r) if r.kind == RevisionKind::Deletion);
                let mut runs = Vec::new();
                for inline in group {
                    runs.extend(self.comment_events(&mut active, &inline.marks.comments));
                    runs.extend(self.content(&inline.content, deleted)?);
                }
                inner.extend(self.wrap_revision(rev.as_ref(), runs));
            }
            out.extend(self.wrap_link(link.as_ref(), inner));
        }
        out.extend(self.comment_events(&mut active, &[]));
        Ok(out)
    }

    /// Range markers for the comments that stop and start between two inlines. Ranges that
    /// continue keep their place; new ones are appended, which is how the reader rebuilds the
    /// list.
    fn comment_events(&mut self, active: &mut Vec<CommentId>, now: &[CommentId]) -> Vec<Node> {
        let mut nodes = Vec::new();
        let ending: Vec<CommentId> = active
            .iter()
            .filter(|id| !now.contains(id))
            .cloned()
            .collect();
        for id in ending.iter().rev() {
            let n = self.ids.comments.number(id.as_str());
            nodes.push(Node::new("w:commentRangeEnd").num("w:id", n));
            nodes.push(Node::new("w:r").child(Node::new("w:commentReference").num("w:id", n)));
        }
        active.retain(|id| now.contains(id));
        for id in now {
            if !active.contains(id) {
                let n = self.ids.comments.number(id.as_str());
                nodes.push(Node::new("w:commentRangeStart").num("w:id", n));
                active.push(id.clone());
            }
        }
        nodes
    }

    fn wrap_revision(&mut self, revision: Option<&Revision>, runs: Vec<Node>) -> Vec<Node> {
        let Some(rev) = revision else { return runs };
        let name = match rev.kind {
            RevisionKind::Insertion => "w:ins",
            RevisionKind::Deletion => "w:del",
        };
        let n = self.ids.revisions.number(rev.id.as_str());
        vec![Node::new(name)
            .num("w:id", n)
            .attr("w:author", &rev.author)
            .attr("w:date", &rev.date)
            .children(runs)]
    }

    fn wrap_link(&mut self, link: Option<&Link>, nodes: Vec<Node>) -> Vec<Node> {
        let Some(link) = link else { return nodes };
        let node = Node::new("w:hyperlink");
        let mut node = if let Some(anchor) = link.href.strip_prefix('#') {
            node.attr("w:anchor", anchor)
        } else {
            let rid = self.rels.add("hyperlink", &link.href, true);
            node.attr("r:id", &rid)
        };
        if let Some(title) = &link.title {
            node = node.attr("w:tooltip", title);
        }
        vec![node.attr("w:history", "1").children(nodes)]
    }
}
