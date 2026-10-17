//! The state while a paragraph is read: the container's open ranges, white space, the
//! enclosing spans' properties, link and deletion; and the uncollapsed text of cached values.

use crate::text::{spaces, Space};
use cdoc_model::{CommentId, Inline, InlineContent, Link, Marks, Revision, RunProperties, TextRun};
use cdoc_pkg::xml::{Child, Node};

/// Ranges open in the container being read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Story {
    /// Open comment ranges: the annotation's name and the comment.
    pub comments: Vec<(String, CommentId)>,
    /// The open insertion.
    pub insertion: Option<Revision>,
}

impl Story {
    /// Nothing open.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            comments: Vec::new(),
            insertion: None,
        }
    }
}

/// The state while reading one paragraph.
#[derive(Debug)]
pub struct Ctx<'s> {
    /// The container's open ranges.
    pub story: &'s mut Story,
    /// White-space state.
    pub space: Space,
    /// Text properties of the enclosing spans.
    pub run: RunProperties,
    /// The enclosing hyperlink.
    pub link: Option<Link>,
    /// The deletion whose removed content is being read.
    pub deletion: Option<Revision>,
}

impl Ctx<'_> {
    fn marks(&self) -> Marks {
        let mut comments: Vec<CommentId> = Vec::new();
        for (_, id) in &self.story.comments {
            if !comments.contains(id) {
                comments.push(id.clone());
            }
        }
        Marks {
            link: self.link.clone(),
            comments,
            revision: self
                .deletion
                .clone()
                .or_else(|| self.story.insertion.clone()),
        }
    }

    /// Append content with the current marks.
    pub fn push(&self, content: InlineContent, out: &mut Vec<Inline>) {
        out.push(Inline {
            content,
            marks: self.marks(),
        });
    }

    pub fn text(&self, text: String, out: &mut Vec<Inline>) {
        if !text.is_empty() {
            let run = TextRun {
                text,
                properties: self.run.clone(),
            };
            self.push(InlineContent::Text(run), out);
        }
    }
}

/// All character data inside, uncollapsed (cached field values are written verbatim).
pub fn raw_text(node: &Node) -> String {
    node.children
        .iter()
        .map(|c| match c {
            Child::Text(t) => t.clone(),
            Child::Node(n) if n.name == "text:s" => spaces(n),
            Child::Node(n) if n.name == "text:tab" => "\t".to_owned(),
            Child::Node(n) if n.name == "text:line-break" => "\n".to_owned(),
            Child::Node(n) => raw_text(n),
        })
        .collect()
}
