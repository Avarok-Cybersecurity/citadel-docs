//! The selection as plain text, for Copy and Cut: characters as they are, `\n` where a
//! paragraph starts (and for a break), `\t` for a tab; images, fields and other objects give
//! nothing, and headers, footers and footnote bodies inside the range are skipped. The result
//! record carries it up to [`MAX_SELECTION_TEXT_BYTES`], because a browser's copy event must
//! set the clipboard synchronously; past that the UI asks [`Editor::selection_text`].

use super::Editor;
use crate::error::EditError;
use cdoc_crdt::{atom_at, Atom, AtomKind, Doc, Unit};
use cdoc_model::InlineContent;
use std::ops::Range;

/// The most UTF-8 bytes of selection text a result record carries.
pub const MAX_SELECTION_TEXT_BYTES: usize = 64 * 1024;

/// The text of `range`, or `None` once it passes `cap` bytes.
fn text_of(
    doc: &Doc,
    range: Range<usize>,
    cap: Option<usize>,
) -> Result<Option<String>, EditError> {
    let mut out = String::new();
    let mut units = doc.units_from(range.start).take(range.len()).enumerate();
    while let Some((k, (_, unit))) = units.next() {
        match unit {
            Unit::Char(c) => out.push(c),
            Unit::Atom(AtomKind::Paragraph) if k > 0 => out.push('\n'),
            Unit::Atom(AtomKind::Inline) => match atom_at(doc, range.start + k)? {
                Some(Atom::Inline(InlineContent::Tab)) => out.push('\t'),
                Some(Atom::Inline(InlineContent::Break { .. })) => out.push('\n'),
                _ => {}
            },
            Unit::Atom(AtomKind::HeaderFooter | AtomKind::Footnote) => {
                let mut depth = 0u32;
                for (_, (_, u)) in units.by_ref() {
                    match u {
                        Unit::Atom(AtomKind::Table) => depth += 1,
                        Unit::Atom(AtomKind::End) if depth == 0 => break,
                        Unit::Atom(AtomKind::End) => depth -= 1,
                        _ => {}
                    }
                }
            }
            Unit::Atom(_) => {}
        }
        if cap.is_some_and(|cap| out.len() > cap) {
            return Ok(None);
        }
    }
    Ok(Some(out))
}

impl Editor<'_> {
    /// The selection's plain text, however long (empty for a caret).
    pub fn selection_text(&self) -> Result<String, EditError> {
        let range = self.selection.range(&self.doc)?;
        Ok(text_of(&self.doc, range, None)?.unwrap_or_default())
    }

    /// The selection's plain text if it is at most [`MAX_SELECTION_TEXT_BYTES`] long.
    pub(crate) fn capped_selection_text(&self) -> Result<Option<String>, EditError> {
        let range = self.selection.range(&self.doc)?;
        text_of(&self.doc, range, Some(MAX_SELECTION_TEXT_BYTES))
    }
}
