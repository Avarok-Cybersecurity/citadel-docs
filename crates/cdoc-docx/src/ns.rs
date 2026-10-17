//! Namespaces and content types shared by every part.

use crate::xml::Node;

/// The `WordprocessingML` main namespace.
pub const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
/// The Word 2010 extension namespace (check boxes, paragraph ids).
pub const W14: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
/// The Word 2012 extension namespace (comment threading).
pub const W15: &str = "http://schemas.microsoft.com/office/word/2012/wordml";

/// Content types by part role.
pub mod content_type {
    /// Styles.
    pub const STYLES: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml";
    /// Numbering.
    pub const NUMBERING: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
    /// Settings.
    pub const SETTINGS: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml";
    /// Footnotes.
    pub const FOOTNOTES: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";
    /// Comments.
    pub const COMMENTS: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml";
    /// Comment threading.
    pub const COMMENTS_EXTENDED: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.commentsExtended+xml";
    /// Header.
    pub const HEADER: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
    /// Footer.
    pub const FOOTER: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml";
    /// Core properties.
    pub const CORE: &str = "application/vnd.openxmlformats-package.core-properties+xml";
}

/// Put the namespace declarations every part carries on a part's root element, so any part
/// can hold any construct without per-part bookkeeping.
#[must_use]
pub fn declare(mut node: Node) -> Node {
    let declarations = [
        ("xmlns:w", W),
        (
            "xmlns:r",
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        ),
        (
            "xmlns:wp",
            "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing",
        ),
        (
            "xmlns:a",
            "http://schemas.openxmlformats.org/drawingml/2006/main",
        ),
        (
            "xmlns:pic",
            "http://schemas.openxmlformats.org/drawingml/2006/picture",
        ),
        (
            "xmlns:mc",
            "http://schemas.openxmlformats.org/markup-compatibility/2006",
        ),
        ("xmlns:w14", W14),
        ("xmlns:w15", W15),
        ("mc:Ignorable", "w14 w15"),
    ];
    let mut attrs: Vec<(String, String)> = declarations
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    attrs.append(&mut node.attrs);
    node.attrs = attrs;
    node
}

/// An empty root element with the declarations.
#[must_use]
pub fn root(name: &str) -> Node {
    declare(Node::new(name))
}
