//! The small enums commands take.

/// A character format that toggles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkKind {
    /// Bold.
    Bold,
    /// Italic.
    Italic,
    /// Single underline.
    Underline,
    /// Strikethrough.
    Strike,
    /// Subscript.
    Subscript,
    /// Superscript.
    Superscript,
}

/// How much a deletion key removes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteUnit {
    /// One cluster (Backspace, Delete).
    Char,
    /// To the word boundary (Ctrl+Backspace, Ctrl+Delete).
    Word,
}
