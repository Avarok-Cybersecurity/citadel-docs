//! The command enum: every document edit the editor runs as one transaction.

use super::kinds::{DeleteUnit, MarkKind};
use crate::position::{Position, Selection};
use cdoc_model::{
    builtin::ListKind, BreakKind, Comment, ContentId, FootnoteId, Length, NumId, PageSetupPatch,
    ParagraphPatch, RunPatch, StyleId,
};

/// An editing command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Type text over the selection; `\n` splits the paragraph and `\t` inserts a tab.
    InsertText(String),
    /// Backspace.
    DeleteBackward(DeleteUnit),
    /// Delete.
    DeleteForward(DeleteUnit),
    /// Enter.
    SplitParagraph,
    /// A line, page or column break.
    InsertBreak(BreakKind),
    /// A tab character.
    InsertTab,
    /// Toggle a character format (on the selection, or for the next typed text).
    ToggleMark(MarkKind),
    /// Change the character properties the patch names; `null` clears one (back to the
    /// style's).
    SetRunProperty(RunPatch),
    /// Change the paragraph properties the patch names, on every selected paragraph.
    SetParagraphProperty(ParagraphPatch),
    /// Apply a paragraph style.
    SetParagraphStyle(StyleId),
    /// Make the selected paragraphs list items of `num_id` at `level`.
    SetNumbering {
        /// The numbering instance.
        num_id: NumId,
        /// The level, `0..=8`.
        level: u8,
    },
    /// Make the selected paragraphs items of a built-in list at `level`: the document's
    /// definition of that template if it has one, a new one (reported as created) if not.
    SetNumberingKind {
        /// The template.
        kind: ListKind,
        /// The level, `0..=8`.
        level: u8,
    },
    /// Take the selected paragraphs out of their list.
    ClearNumbering,
    /// Change the page setup of every section the selection touches.
    SetSectionProperty(PageSetupPatch),
    /// One list level deeper, or one tab stop more left indent.
    Indent,
    /// One list level shallower, or one tab stop less left indent.
    Outdent,
    /// A `rows` × `cols` table at the caret.
    InsertTable {
        /// Rows, at least one.
        rows: u32,
        /// Columns, at least one.
        cols: u32,
    },
    /// A row above the caret's row.
    InsertRowAbove,
    /// A row below the caret's row.
    InsertRowBelow,
    /// A column left of the caret's column.
    InsertColumnLeft,
    /// A column right of the caret's column.
    InsertColumnRight,
    /// Delete the caret's row.
    DeleteRow,
    /// Delete the caret's column.
    DeleteColumn,
    /// Delete the caret's table.
    DeleteTable,
    /// Merge the cells from the anchor's to the head's into one.
    MergeCells,
    /// An inline image.
    InsertImage {
        /// The image bytes' content id.
        content_id: ContentId,
        /// Display width and height; `None` is the image's own size at 96 pixels per inch,
        /// scaled down to the column width keeping its aspect ratio (the editor must hold it).
        size: Option<(Length, Length)>,
    },
    /// A footnote reference at the caret and an empty footnote body, with the caller's id
    /// (document-local, unique) or, for `None`, one the editor picks and reports as created.
    InsertFootnote(Option<FootnoteId>),
    /// Link the selection (or insert the URL as linked text).
    InsertLink(String),
    /// Remove the link from the selection or the link around the caret.
    RemoveLink,
    /// Anchor a comment (id, author, date from the caller) on the selection or the word at
    /// the caret.
    InsertComment(Comment),
    /// Replace the gaps from one position to another with text (find and replace).
    Replace {
        /// Start.
        from: Position,
        /// End.
        to: Position,
        /// The replacement.
        text: String,
    },
    /// Select the whole body.
    SelectAll,
    /// Delete the selection as its own undo step, after the UI copied its text (the result
    /// record's or `Editor::selection_text`). A caret has nothing to cut.
    Cut,
    /// Set the selection.
    SetSelection(Selection),
}
