//! Markdown for the Citadel Docs model: `CommonMark` with GFM tables, strikethrough and
//! footnotes, in and out. Pure: text in, text out.
//!
//! Markdown is lossy by nature. [`document_to_md`] and [`md_to_document`] each return a
//! [`cdoc_model::Report`] naming every kind of thing that did not map, counted. This is what
//! does map.
//!
//! # Model to Markdown
//!
//! | Model | Markdown |
//! |---|---|
//! | paragraph, style `Heading1`..`Heading6` | `#`..`######` (`Title` is written as level 1 and reported) |
//! | paragraph, style `Quote` | `> ` |
//! | paragraph in a list | `- ` for bullets, `N.` for numbered (counted from the list's start); nesting is four spaces per level, never deeper than the item above allows |
//! | paragraph with a bottom border and no content | `---` |
//! | table | GFM pipe table; the first row is the header; column alignment from the first body row's paragraphs; cells hold inline text, several paragraphs joined by `<br>` |
//! | direct bold, italic, strikethrough | `**`, `*`, `~~`; where two delimiter runs would touch, `<strong>`, `<em>`, `<del>` |
//! | run in a monospace font (`Courier New`, `Consolas`, `Courier`, `Menlo`, `Monaco`) | code span |
//! | hyperlink | `[text](<href> "title")` |
//! | image | `![alt](<cid:CONTENT-ID>)`; the reader needs the size from its options |
//! | footnote reference and body | `[^N]` and `[^N]: ...` |
//! | line break | backslash, newline |
//! | text | escaped so it reads back as text |
//!
//! Reported, not written: document title, section page setup, headers, footers, comments,
//! tracked deletions (the text is left out) and insertions (accepted), underline, colour,
//! highlight, size, font family, super/subscript, capitals, page and column breaks, merged and
//! nested cells (flattened), form fields (their value), signature fields, paragraph formatting
//! (alignment, spacing, indent, borders, shading, pagination), custom paragraph styles, empty
//! paragraphs. Emphasis on punctuation at the edge of a run is moved outside its markers,
//! because Markdown cannot open emphasis there; that is reported too. Only direct formatting is
//! written, not what a style implies.
//!
//! # Markdown to model
//!
//! The reader builds on the built-in styles. Headings, quotes and lists get the styles above;
//! bullet lists share one numbering instance and each numbered list gets its own, so each
//! counts from its own start. Tables are given single borders and equal columns sharing
//! [`MdReadOptions::text_width`]. Soft line breaks become spaces. A code block becomes a
//! monospace paragraph (reported), HTML blocks are dropped (reported), inline HTML is dropped
//! except `<br>`, `<del>`, `<s>`, `<strike>`, `<strong>`, `<b>`, `<em>`, `<i>`. An image whose
//! URL is not `cid:` keeps its alt text (reported). There are no defaults: the caller supplies
//! the sizes in [`MdReadOptions`].

#![forbid(unsafe_code)]

mod options;
mod read;
mod write;

pub use options::MdReadOptions;
pub use read::md_to_document;
pub use write::{document_to_md, CODE_FONT, MONOSPACE};
