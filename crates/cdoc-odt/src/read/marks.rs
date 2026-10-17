//! Annotations as comments (a range when an `office:annotation-end` names them, a point
//! otherwise) and notes as footnotes.

use super::ctx::{raw_text, Ctx};
use super::Reader;
use crate::text::Space;
use cdoc_model::{Comment, CommentId, Footnote, FootnoteId, Inline, InlineContent};
use cdoc_pkg::xml::{Child, Node};
use cdoc_pkg::PackageError;

/// A paragraph's plain text, collapsed the way the body's is.
fn plain(p: &Node, space: &mut Space, out: &mut String) {
    for child in &p.children {
        match child {
            Child::Text(t) => out.push_str(&space.collapse(t)),
            Child::Node(n) => match n.name.as_str() {
                "text:s" => {
                    out.push_str(&crate::text::spaces(n));
                    space.white();
                }
                "text:tab" | "text:line-break" => {
                    out.push(if n.name == "text:tab" { '\t' } else { '\n' });
                    space.white();
                }
                _ => plain(n, space, out),
            },
        }
    }
}

impl Reader<'_> {
    fn comment_text(&mut self, node: &Node) -> String {
        let mut lines = Vec::new();
        for child in node.elems() {
            match child.name.as_str() {
                "dc:creator" | "dc:date" => {}
                "text:p" | "text:h" => {
                    let mut line = String::new();
                    plain(child, &mut Space::start(), &mut line);
                    lines.push(line);
                }
                other => {
                    self.log
                        .lose(&format!("{other} in a comment read as its text"));
                    lines.push(raw_text(child));
                }
            }
        }
        lines.join("\n")
    }

    /// `office:annotation`: a comment, opened as a range when its end comes later.
    pub(super) fn annotation(&mut self, node: &Node, ctx: &mut Ctx<'_>) {
        let name = node.get("office:name").map(str::to_owned);
        if let Some(id) = name
            .as_ref()
            .and_then(|n| self.sidecar.segments.get(n))
            .cloned()
        {
            if let Some(n) = name {
                ctx.story.comments.push((n, id));
            }
            return;
        }
        let id = match &name {
            Some(n) if !self.comments.iter().any(|c| c.id.as_str() == n) => {
                CommentId::new(n.as_str())
            }
            _ => loop {
                self.unnamed += 1;
                let candidate = CommentId::new(format!("odt-comment-{}", self.unnamed));
                if !self.comments.iter().any(|c| c.id == candidate) {
                    break candidate;
                }
            },
        };
        let field = |key: &str| node.find(key).map(Node::content).unwrap_or_default();
        let text = self.comment_text(node);
        self.comments.push(Comment {
            id: id.clone(),
            author: field("dc:creator"),
            date: field("dc:date"),
            text,
            resolved: false,
            parent: None,
        });
        match name.filter(|n| self.ends.contains(n)) {
            Some(n) => ctx.story.comments.push((n, id)),
            None if self.sidecar.comments.iter().any(|c| c.id == id) => {}
            None => self.log.lose("a comment on a point kept without an anchor"),
        }
    }

    /// `office:annotation-end`: the named range closes.
    pub(super) fn annotation_end(node: &Node, ctx: &mut Ctx<'_>) {
        if let Some(name) = node.get("office:name") {
            ctx.story.comments.retain(|(n, _)| n != name);
        }
    }

    fn footnote_id(&self, name: Option<&str>) -> FootnoteId {
        let taken = |id: u32| self.footnotes.iter().any(|f| f.id.0 == id);
        let digits = name.map(|n| n.trim_start_matches(|c: char| !c.is_ascii_digit()));
        if let Some(id) = digits
            .and_then(|d| d.parse::<u32>().ok())
            .filter(|id| !taken(*id))
        {
            return FootnoteId(id);
        }
        let mut id = self
            .footnotes
            .iter()
            .map(|f| f.id.0)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        while taken(id) {
            id = id.saturating_add(1);
        }
        FootnoteId(id)
    }

    /// `text:note`: a footnote with its mark (endnotes become footnotes).
    pub(super) fn note(
        &mut self,
        node: &Node,
        ctx: &mut Ctx<'_>,
        out: &mut Vec<Inline>,
    ) -> Result<(), PackageError> {
        if node.get("text:note-class") == Some("endnote") {
            self.log.lose("an endnote read as a footnote");
        }
        if node
            .find("text:note-citation")
            .and_then(|c| c.get("text:label"))
            .is_some()
        {
            self.log.lose("a footnote's custom mark");
        }
        let name = node.get("text:id");
        let id = self.footnote_id(name);
        if let Some(name) = name {
            self.note_names.insert(name.to_owned(), id);
        }
        // The id is taken before the body is read, so notes inside it get later ids.
        let at = self.footnotes.len();
        self.footnotes.push(Footnote {
            id,
            blocks: Vec::new(),
        });
        let blocks = match node.find("text:note-body") {
            Some(body) => self.container(body.elems())?.blocks,
            None => Vec::new(),
        };
        self.footnotes[at].blocks = blocks;
        if !self.sidecar.orphans.contains(&id) {
            ctx.push(InlineContent::FootnoteRef { id }, out);
        }
        ctx.space.visible();
        Ok(())
    }

    /// `text:note-ref`: another mark of a note already read.
    pub(super) fn note_ref(&mut self, node: &Node, ctx: &mut Ctx<'_>, out: &mut Vec<Inline>) {
        if let Some(id) = node
            .get("text:ref-name")
            .and_then(|n| self.note_names.get(n))
        {
            ctx.push(InlineContent::FootnoteRef { id: *id }, out);
        } else {
            self.log
                .lose("a reference to a note not yet read, read as its text");
            ctx.text(raw_text(node), out);
        }
        ctx.space.visible();
    }
}
