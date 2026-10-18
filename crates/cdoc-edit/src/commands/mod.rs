//! The commands: one enum, each variant one CRDT transaction. A command reads the document as
//! it stood before it (the model view and the outline) and edits through a [`Tx`], which
//! records how to undo each edit.

mod command;
mod delete;
mod format;
mod kinds;
mod lists;
mod objects;
mod section;
mod table;
mod table_delete;
mod table_grid;
mod table_merge;
mod typing;

use crate::error::EditError;
use crate::outline::Outline;
use crate::position::{Position, Selection};
use crate::tx::Tx;
use cdoc_model::{Document, FootnoteId, NumId, ParagraphProperties, RunProperties};
use std::borrow::Cow;

pub use command::Command;
pub(crate) use format::{selected_paragraphs, texts};
pub use kinds::{DeleteUnit, MarkKind};
pub(crate) use typing::{inherited_by, props_at};

/// How a command's edit joins the undo history.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Group {
    /// Typing: joins the previous typing of the same class (word characters or not).
    Typing { word: bool },
    /// Its own undo step.
    Own,
}

/// What a command reads and edits.
pub(crate) struct Ctx<'a, 'd> {
    pub(crate) tx: Tx<'d>,
    pub(crate) model: &'a Document,
    /// The outline as of the last [`Ctx::refresh`].
    pub(crate) outline: Cow<'a, Outline>,
    pub(crate) selection: Selection,
    pub(crate) pending: Option<RunProperties>,
}

/// Something a command made under an id the editor picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Created {
    /// A footnote.
    Footnote(FootnoteId),
    /// A numbering instance (and the definition it uses).
    Numbering(NumId),
}

/// A command's result.
pub(crate) struct Outcome {
    pub(crate) selection: Selection,
    pub(crate) pending: Option<RunProperties>,
    pub(crate) group: Group,
    pub(crate) created: Vec<Created>,
}

/// An own undo step ending with `selection`.
pub(crate) const fn done(selection: Selection) -> Outcome {
    Outcome {
        selection,
        pending: None,
        group: Group::Own,
        created: Vec::new(),
    }
}

impl Ctx<'_, '_> {
    pub(crate) fn caret_at(&self, gap: usize) -> Result<Selection, EditError> {
        let g = crate::position::caret_gap(self.tx.doc, gap)?;
        Position::at(self.tx.doc, g)
            .map(Selection::caret)
            .ok_or(EditError::UnknownPosition)
    }

    /// Rebuilds the outline if this command changed the structure since it was built.
    pub(crate) fn refresh(&mut self) {
        if self.tx.structural {
            self.outline = Cow::Owned(Outline::build(&self.tx.doc.structure()));
            self.tx.structural = false;
        }
    }

    pub(crate) fn require_regular(&self) -> Result<(), EditError> {
        if self.outline.regular {
            Ok(())
        } else {
            Err(EditError::Irregular)
        }
    }
}

/// Runs `command`.
pub(crate) fn run(ctx: &mut Ctx<'_, '_>, command: Command) -> Result<Outcome, EditError> {
    match command {
        Command::InsertText(text) => typing::insert_text(ctx, &text),
        Command::DeleteBackward(unit) => delete::backward(ctx, unit),
        Command::DeleteForward(unit) => delete::forward(ctx, unit),
        Command::SplitParagraph => typing::split(ctx),
        Command::InsertBreak(kind) => {
            typing::inline(ctx, cdoc_model::InlineContent::Break { kind })
        }
        Command::InsertTab => typing::inline(ctx, cdoc_model::InlineContent::Tab),
        Command::ToggleMark(kind) => format::toggle(ctx, kind),
        Command::SetRunProperty(patch) => format::set_run(ctx, &patch),
        Command::SetParagraphProperty(patch) => format::paragraphs(ctx, |p| patch.apply(p)),
        Command::SetParagraphStyle(style) => format::paragraphs(ctx, |p| ParagraphProperties {
            style: Some(style.clone()),
            ..p.clone()
        }),
        Command::SetNumbering { num_id, level } => lists::set(ctx, num_id, level),
        Command::SetNumberingKind { kind, level } => lists::set_kind(ctx, kind, level),
        Command::ClearNumbering => format::paragraphs(ctx, |p| ParagraphProperties {
            numbering: None,
            ..p.clone()
        }),
        Command::SetSectionProperty(patch) => section::set_page(ctx, &patch),
        Command::Indent => lists::indent(ctx, true),
        Command::Outdent => lists::indent(ctx, false),
        Command::InsertTable { rows, cols } => table::insert(ctx, rows, cols),
        Command::InsertRowAbove => table_grid::insert_row(ctx, false),
        Command::InsertRowBelow => table_grid::insert_row(ctx, true),
        Command::InsertColumnLeft => table_grid::insert_column(ctx, false),
        Command::InsertColumnRight => table_grid::insert_column(ctx, true),
        Command::DeleteRow => table_delete::delete_row(ctx),
        Command::DeleteColumn => table_delete::delete_column(ctx),
        Command::DeleteTable => table::delete(ctx),
        Command::MergeCells => table_merge::merge(ctx),
        Command::InsertImage { content_id, size } => {
            let (width, height) = size.ok_or(EditError::NotApplicable(
                "the image's size is decided by the editor",
            ))?;
            objects::image(ctx, content_id, width, height)
        }
        Command::InsertFootnote(id) => objects::footnote(ctx, id),
        Command::InsertLink(href) => objects::link(ctx, href),
        Command::RemoveLink => objects::unlink(ctx),
        Command::InsertComment(comment) => objects::comment(ctx, comment),
        Command::Replace { from, to, text } => typing::replace(ctx, from, to, &text),
        Command::SelectAll => typing::select_all(ctx),
        Command::SetSelection(selection) => typing::set_selection(ctx, selection),
    }
}
