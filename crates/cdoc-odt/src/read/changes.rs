//! Tracked changes: the change regions (found before the body is read, since a mark can come
//! before its region), insertions as ranges between `text:change-start` and `text:change-end`,
//! deletions as the removed content a `text:change` point stands for.

use super::ctx::Ctx;
use super::Reader;
use crate::text::Space;
use cdoc_model::{Inline, Revision, RevisionId, RevisionKind};
use cdoc_pkg::xml::Node;
use cdoc_pkg::PackageError;

/// A change region.
#[derive(Debug, Clone)]
pub struct Region<'a> {
    /// The tracked change.
    pub revision: Revision,
    /// For a deletion, the `text:deletion` element holding the removed content.
    pub deleted: Option<&'a Node>,
}

fn info(change: &Node) -> (String, String) {
    let info = change.find("office:change-info");
    let field = |name: &str| {
        info.and_then(|i| i.find(name))
            .map(Node::content)
            .unwrap_or_default()
    };
    (field("dc:creator"), field("dc:date"))
}

impl<'a> Reader<'a> {
    /// Record a `text:changed-region`.
    pub(super) fn region(&mut self, node: &'a Node) {
        let Some(id) = node.get("text:id") else {
            self.log.lose("a change region without an id");
            return;
        };
        let Some(change) = node.elems().next() else {
            self.log.lose("an empty change region");
            return;
        };
        let kind = match change.name.as_str() {
            "text:insertion" => RevisionKind::Insertion,
            "text:deletion" => RevisionKind::Deletion,
            "text:format-change" => {
                self.log
                    .lose("a tracked formatting change; the formatting is read as current");
                self.formats.insert(id.to_owned());
                return;
            }
            other => {
                self.log.lose(&format!("a change region holding {other}"));
                return;
            }
        };
        let (author, date) = info(change);
        let revision = Revision {
            id: self
                .sidecar
                .regions
                .get(id)
                .cloned()
                .unwrap_or_else(|| RevisionId::new(id)),
            kind,
            author,
            date,
        };
        let deleted = (kind == RevisionKind::Deletion).then_some(change);
        self.regions
            .insert(id.to_owned(), Region { revision, deleted });
    }

    fn change_of(&mut self, node: &Node) -> Option<Region<'a>> {
        let id = node.get("text:change-id");
        let found = id.and_then(|id| self.regions.get(id)).cloned();
        if found.is_none() && !id.is_some_and(|id| self.formats.contains(id)) {
            self.log
                .lose("a change mark whose region is missing or unread");
        }
        found
    }

    /// `text:change-start`: an insertion opens.
    pub(super) fn change_start(&mut self, node: &Node, ctx: &mut Ctx<'_>) {
        match self.change_of(node) {
            Some(Region {
                revision,
                deleted: None,
            }) => ctx.story.insertion = Some(revision),
            Some(_) => self
                .log
                .lose("a deletion marked as a range; its removed content dropped"),
            None => {}
        }
    }

    /// `text:change-end`: the insertion closes.
    pub(super) fn change_end(ctx: &mut Ctx<'_>) {
        ctx.story.insertion = None;
    }

    /// `text:change`: the removed content of a deletion, read in place.
    pub(super) fn change(
        &mut self,
        node: &Node,
        ctx: &mut Ctx<'_>,
        out: &mut Vec<Inline>,
    ) -> Result<(), PackageError> {
        let Some(region) = self.change_of(node) else {
            return Ok(());
        };
        let Some(deleted) = region.deleted else {
            self.log.lose("an insertion marked as a point");
            return Ok(());
        };
        // The writer encodes removed content apart from the line around it.
        let saved = (
            ctx.deletion.replace(region.revision),
            ctx.link.take(),
            std::mem::replace(&mut ctx.space, Space::start()),
        );
        let mut paragraphs = 0;
        for child in deleted.elems() {
            match child.name.as_str() {
                "office:change-info" => {}
                "text:p" | "text:h" => {
                    paragraphs += 1;
                    if paragraphs == 2 {
                        self.log.lose(
                            "a deletion across paragraphs read as deleted text in one paragraph",
                        );
                    }
                    self.inlines(child, ctx, out)?;
                }
                other => self.log.lose(&format!("{other} inside a deletion")),
            }
        }
        (ctx.deletion, ctx.link, ctx.space) = saved;
        Ok(())
    }
}
