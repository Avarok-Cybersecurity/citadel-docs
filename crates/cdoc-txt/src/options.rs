//! Reader options. There is no default: whether a single newline inside a paragraph means a
//! line break or a wrapped line is a property of the text, and only the caller knows it.

/// What a single newline inside a paragraph means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleNewline {
    /// A line break: the text is laid out as typed (poetry, addresses, code).
    Break,
    /// A space: the text is hard-wrapped prose.
    Space,
}

/// How to turn plain text into a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TxtReadOptions {
    /// The meaning of a single newline within a paragraph.
    pub single_newline: SingleNewline,
}
