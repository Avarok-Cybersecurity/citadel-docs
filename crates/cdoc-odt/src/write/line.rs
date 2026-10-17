//! The content of one paragraph as it is built: hyperlinks grouped over adjacent inlines,
//! white space encoded with one state, and the ranges (comments, insertions) that stay open
//! from one paragraph to the next within a container.

use crate::text::Space;
use cdoc_model::{CommentId, Link, Revision};
use cdoc_pkg::xml::{Child, Node};

/// Ranges open in the container being written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Story {
    /// Comments whose annotation has started, with the annotation's name, in opening order.
    pub comments: Vec<(CommentId, String)>,
    /// The insertion being written and its change region.
    pub insertion: Option<(Revision, String)>,
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

/// One paragraph's content.
#[derive(Debug)]
pub struct Line {
    children: Vec<Child>,
    link: Option<(Link, Vec<Child>)>,
    /// White-space state.
    pub space: Space,
    /// The deletion region whose marker was the last thing written, which a following deleted
    /// inline of the same revision joins.
    pub deletion: Option<usize>,
}

fn link_node(link: &Link, children: Vec<Child>) -> Node {
    let mut node = Node::new("text:a")
        .attr("xlink:type", "simple")
        .attr("xlink:href", &link.href);
    if let Some(title) = &link.title {
        node = node.attr("office:title", title);
    }
    node.children = children;
    node
}

impl Line {
    /// An empty paragraph.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            children: Vec::new(),
            link: None,
            space: Space::start(),
            deletion: None,
        }
    }

    /// Append content, inside the open hyperlink if there is one.
    pub fn push(&mut self, child: Child) {
        self.deletion = None;
        self.push_keeping_deletion(child);
    }

    /// Append the marker that opens a deletion group (which stays joinable).
    pub fn push_keeping_deletion(&mut self, child: Child) {
        match &mut self.link {
            Some((_, children)) => children.push(child),
            None => self.children.push(child),
        }
    }

    /// Append an element.
    pub fn element(&mut self, node: Node) {
        self.push(Child::Node(node));
    }

    /// Make `link` the hyperlink of what follows, closing a different one.
    pub fn link_to(&mut self, link: Option<&Link>) {
        if self.link.as_ref().map(|(l, _)| l) == link {
            return;
        }
        self.close_link();
        if let Some(link) = link {
            self.link = Some((link.clone(), Vec::new()));
        }
    }

    fn close_link(&mut self) {
        if let Some((link, children)) = self.link.take() {
            self.children.push(Child::Node(link_node(&link, children)));
            self.deletion = None;
        }
    }

    /// The finished content.
    #[must_use]
    pub fn finish(mut self) -> Vec<Child> {
        self.close_link();
        self.children
    }
}
