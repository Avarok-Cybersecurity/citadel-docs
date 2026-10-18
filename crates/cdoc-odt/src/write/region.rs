//! A change region as it is collected: the revision, and for a deletion the removed content.

use super::notes::info;
use cdoc_model::{Revision, RevisionKind};
use cdoc_pkg::xml::{Child, Node};

/// A change region being collected.
#[derive(Debug, Clone)]
pub struct Region {
    /// `text:id`.
    pub id: String,
    /// The tracked change.
    pub revision: Revision,
    /// For a deletion, the removed content (one paragraph's worth).
    pub deleted: Option<Vec<Child>>,
}

impl Region {
    /// The `text:changed-region` element.
    #[must_use]
    pub fn node(&self) -> Node {
        let change_info = Node::new("office:change-info")
            .children(info(&self.revision.author, &self.revision.date));
        let change = match &self.deleted {
            Some(content) => {
                let mut p = Node::new("text:p");
                p.children.clone_from(content);
                Node::new("text:deletion").child(change_info).child(p)
            }
            None => Node::new("text:insertion").child(change_info),
        };
        Node::new("text:changed-region")
            .attr("text:id", &self.id)
            .child(change)
    }
}

/// Whether a revision is a deletion.
#[must_use]
pub fn is_deletion(revision: Option<&Revision>) -> bool {
    revision.is_some_and(|r| r.kind == RevisionKind::Deletion)
}
