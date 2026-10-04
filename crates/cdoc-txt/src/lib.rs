//! Plain text for the Citadel Docs model, in and out. Pure: text in, text out.
//!
//! # Text to model
//!
//! Paragraphs are separated by blank lines. A tab is a tab and a form feed (U+000C) a page
//! break; line endings are normalised. Whether a single newline inside a paragraph is a line
//! break or a space is the caller's choice ([`TxtReadOptions`]). Nothing is lost.
//!
//! # Model to text
//!
//! | Model | Text |
//! |---|---|
//! | paragraph | its text, blank line between paragraphs |
//! | list item | two spaces per level, then the computed label (`1.`, `a)`, `1.2`; `-` for bullets), then the text |
//! | style `Quote` | each line prefixed `> ` |
//! | table | columns padded to width, `" | "` between, a `---+---` rule under a header row; right and centred cells are padded to match |
//! | hyperlink | the text, then ` (href)` when it differs from the target |
//! | line break, page break | newline, form feed |
//! | footnote | `[N]` where referenced, `[N] text` after the body |
//! | image, signature field | `[image: alt]`, `[signature: label]` placeholders |
//! | field, form field | the cached text, the value (`[x]`/`[ ]` for check boxes) |
//!
//! Reported, not written: all character and paragraph formatting, headers, footers, comments,
//! tracked deletions (removed) and insertions (accepted), merged cells (text stays in the first
//! position), nested tables (flattened), empty paragraphs.

#![forbid(unsafe_code)]

mod options;
mod read;
mod write;

pub use options::{SingleNewline, TxtReadOptions};
pub use read::txt_to_document;
pub use write::document_to_txt;
