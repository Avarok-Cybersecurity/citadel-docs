//! Annotations, footnotes and change regions: the constructs that refer to content written
//! elsewhere (a comment's text, a footnote's body, a deletion's removed text).

use super::line::{Line, Story};
use super::region::Region;
use super::Writer;
use crate::text::Space;
use cdoc_model::{BreakKind, Comment, CommentId, FootnoteId, Inline, InlineContent, Revision};
use cdoc_pkg::xml::{Child, Node};
use cdoc_pkg::PackageError;

pub(super) fn info(author: &str, date: &str) -> [Node; 2] {
    [
        Node::new("dc:creator").text(author),
        Node::new("dc:date").text(date),
    ]
}

/// A paragraph holding plain text, white space preserved.
pub fn plain_paragraph(text: &str) -> Node {
    let mut p = Node::new("text:p");
    let mut space = Space::start();
    let mut children: Vec<Child> = Vec::new();
    space.encode(text, &mut children);
    p.children = children;
    p
}

impl Writer<'_> {
    fn comment(&self, id: &CommentId) -> Option<&Comment> {
        self.doc.comments.iter().find(|c| &c.id == id)
    }

    /// An annotation for a comment: its first one carries the comment's own name; a further
    /// range of the same comment gets a fresh name recorded in the sidecar.
    pub(super) fn annotation(&mut self, id: &CommentId) -> (Node, String) {
        let name = if self.comments_written.insert(id.clone()) {
            id.as_str().to_owned()
        } else {
            self.log
                .lose("a comment anchored on separate ranges written as one annotation per range");
            loop {
                self.segments += 1;
                let candidate = format!("{}~{}", id.as_str(), self.segments);
                if self.comment(&CommentId::new(candidate.clone())).is_none() {
                    self.sidecar.segments.insert(candidate.clone(), id.clone());
                    break candidate;
                }
            }
        };
        let mut node = Node::new("office:annotation").attr("office:name", &name);
        if let Some(c) = self.comment(id) {
            node = node.children(info(&c.author, &c.date));
            for line in c.text.split('\n') {
                node = node.child(plain_paragraph(line));
            }
        }
        (node, name)
    }

    /// The point annotations for comments no inline is anchored on.
    pub(super) fn unanchored_comments(&mut self) -> Vec<Node> {
        // A page or column break becomes a paragraph boundary, which no annotation can span.
        let anchored: std::collections::BTreeSet<&CommentId> = self
            .doc
            .inlines()
            .filter(|i| {
                !matches!(
                    i.content,
                    InlineContent::Break {
                        kind: BreakKind::Page | BreakKind::Column
                    }
                )
            })
            .flat_map(|i| i.marks.comments.iter())
            .collect();
        let loose: Vec<CommentId> = self
            .doc
            .comments
            .iter()
            .filter(|c| !anchored.contains(&c.id))
            .map(|c| c.id.clone())
            .collect();
        loose.iter().map(|id| self.annotation(id).0).collect()
    }

    /// Bring the open comments to `wanted`: close those no longer wanted, open new ones.
    pub(super) fn close_comments(wanted: &[CommentId], story: &mut Story, line: &mut Line) {
        story.comments.retain(|(id, name)| {
            let keep = wanted.contains(id);
            if !keep {
                line.element(Node::new("office:annotation-end").attr("office:name", name));
            }
            keep
        });
    }

    pub(super) fn open_comments(
        &mut self,
        wanted: &[CommentId],
        story: &mut Story,
        line: &mut Line,
    ) {
        for id in wanted {
            if !story.comments.iter().any(|(open, _)| open == id) {
                let (node, name) = self.annotation(id);
                line.element(node);
                story.comments.push((id.clone(), name));
            }
        }
    }

    fn region(&mut self, revision: &Revision, deleted: bool) -> String {
        self.region_count += 1;
        let id = format!("ct{}", self.region_count);
        self.sidecar.regions.insert(id.clone(), revision.id.clone());
        self.regions.push(Region {
            id: id.clone(),
            revision: revision.clone(),
            deleted: deleted.then(Vec::new),
        });
        id
    }

    /// Close the open insertion if `wanted` differs.
    pub(super) fn close_insertion(wanted: Option<&Revision>, story: &mut Story, line: &mut Line) {
        if story.insertion.as_ref().map(|(r, _)| r) != wanted {
            if let Some((_, id)) = story.insertion.take() {
                line.element(Node::new("text:change-end").attr("text:change-id", &id));
            }
        }
    }

    pub(super) fn open_insertion(
        &mut self,
        wanted: Option<&Revision>,
        story: &mut Story,
        line: &mut Line,
    ) {
        if let (Some(revision), None) = (wanted, &story.insertion) {
            let id = self.region(revision, false);
            line.element(Node::new("text:change-start").attr("text:change-id", &id));
            story.insertion = Some((revision.clone(), id));
        }
    }

    /// A deleted inline: its content goes into a deletion region (joining the previous one when
    /// nothing came between), a `text:change` marks the place.
    pub(super) fn deleted(
        &mut self,
        inline: &Inline,
        revision: &Revision,
        line: &mut Line,
    ) -> Result<(), PackageError> {
        let joinable = line
            .deletion
            .filter(|&i| self.regions.get(i).is_some_and(|r| &r.revision == revision));
        let index = if let Some(i) = joinable {
            i
        } else {
            let id = self.region(revision, true);
            line.push_keeping_deletion(Child::Node(
                Node::new("text:change").attr("text:change-id", &id),
            ));
            self.regions.len() - 1
        };
        line.deletion = Some(index);
        let mut inner = Line::new();
        inner.link_to(inline.marks.link.as_ref());
        self.content(&inline.content, &mut inner)?;
        let children = inner.finish();
        if let Some(content) = self.regions[index].deleted.as_mut() {
            content.extend(children);
        }
        Ok(())
    }

    /// A footnote mark: the note with its body the first time, a note reference after.
    pub(super) fn footnote(&mut self, id: FootnoteId, line: &mut Line) -> Result<(), PackageError> {
        let Some(footnote) = self.doc.footnotes.iter().find(|f| f.id == id) else {
            self.log
                .lose("a footnote reference with no footnote; dropped");
            return Ok(());
        };
        let name = format!("ftn{}", id.0);
        if !self.footnotes_written.insert(id) {
            self.log.lose(
                "a footnote referenced more than once; later references written as note references",
            );
            let reference = Node::new("text:note-ref")
                .attr("text:note-class", "footnote")
                .attr("text:reference-format", "text")
                .attr("text:ref-name", &name);
            line.element(reference.text(&self.citations.to_string()));
            line.space.visible();
            return Ok(());
        }
        self.citations += 1;
        let body = self.container(&footnote.blocks.clone(), None)?;
        let element = Node::new("text:note")
            .attr("text:id", &name)
            .attr("text:note-class", "footnote")
            .child(Node::new("text:note-citation").text(&self.citations.to_string()))
            .child(Node::new("text:note-body").children(body));
        line.element(element);
        line.space.visible();
        Ok(())
    }
}
