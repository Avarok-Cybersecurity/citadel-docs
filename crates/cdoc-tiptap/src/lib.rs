//! The old chat editor's documents, read into the Citadel Docs model once so they can move to
//! the canvas. That editor was TipTap v2 with `StarterKit` and nothing else
//! (`citadel-workspaces/src/components/p2p/editor-extensions.ts`); its documents are
//! ProseMirror JSON in that schema, which is not the model's (`docs/tiptap-schema.md`). Pure:
//! JSON in, a document and a [`Report`] out.
//!
//! # Mapping
//!
//! | `StarterKit` | Model | Loss |
//! |---|---|---|
//! | `doc` | the body of a blank document (built-in styles, A4) | an empty `doc`: one empty paragraph, `info` |
//! | `paragraph` | paragraph, style `Normal` | none |
//! | `heading` (`level` 1 to 6) | paragraph, style `Heading1`..`Heading6` | another level: the nearest, `minor` |
//! | `bulletList` / `orderedList` / `listItem` | flat numbered paragraphs, style `ListParagraph`; all bullet lists share one list, each ordered list counts from its `start`; the nesting depth is the level | a level past the ninth: the ninth, `minor`; a `start` that is not a whole number: 1, `minor`; a `type` other than `1 a A i I`: decimal, `minor` |
//! | an item's further paragraphs | unnumbered, indented to the item's level | none |
//! | `blockquote` | its paragraphs in style `Quote` | `minor` (Quote is centred italics, not an indented bar); a heading, list or code block in it keeps its own style, `minor`; a quote in a quote is one quote, `minor` |
//! | `codeBlock` | one `Normal` paragraph in Courier New, lines as line breaks | `minor`; its `language`, `info` |
//! | `horizontalRule` | an empty paragraph with a bottom border | none |
//! | `hardBreak` | line break | formatting on it, `info` |
//! | `text` | text run | none |
//! | `bold`, `italic`, `strike` | `bold`, `italic`, `strike` on the run | none |
//! | `code` | the run in Courier New | its shading, `minor` |
//!
//! Nothing is refused but input that is not JSON, or whose root is not a node. A node or mark
//! of another type, an attribute `StarterKit` does not have, a block where text belongs or the
//! reverse, a malformed node: each is read as well as it can be, its text kept, and reported
//! (`cdoc_model::loss`, the `loss/tiptap.rs` sentences).

#![forbid(unsafe_code)]

mod blocks;
mod inline;
mod lists;
mod node;
mod place;

#[cfg(feature = "test-support")]
pub mod test_support;

use cdoc_model::{Document, Report};

/// Where the losses are reported: the report's `part`.
pub const PART: &str = "chat document";

/// Why a chat document could not be read at all.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReadError {
    /// The input is not JSON.
    #[error("the chat document is not JSON: {0}")]
    Json(String),
    /// The input is JSON but its root is not a node (an object with a string `type`).
    #[error("the chat document's root is not a TipTap node")]
    NotANode,
}

/// A chat document (TipTap `StarterKit` JSON) as a model document, normalised, and what it
/// lost. The same input always gives the same document and report.
pub fn tiptap_to_document(json: &str) -> Result<(Document, Report), ReadError> {
    let value: serde_json::Value =
        serde_json::from_str(json).map_err(|e| ReadError::Json(e.to_string()))?;
    let root = node::Node::of(&value).ok_or(ReadError::NotANode)?;
    Ok(blocks::read(root))
}

/// A node, mark or attribute name as it appears in a loss sentence: one short word, so a
/// sentence stays classifiable and a hostile name cannot flood the report.
pub(crate) fn name(raw: &str) -> String {
    let mut out: String = raw
        .chars()
        .take(48)
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if out.is_empty() {
        out.push('_');
    }
    out
}
