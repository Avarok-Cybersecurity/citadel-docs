//! Requests: every [`Command`] plus the editor operations that are not document edits (undo,
//! caret moves, composition), each an opcode and its fields.

use super::{Reader, Writer};
use crate::commands::{Command, DeleteUnit, MarkKind};
use crate::position::{Direction, Granularity, Position};
use cdoc_crdt::DecodeError;
use cdoc_model::builtin::ListKind;
use cdoc_model::BreakKind;

/// Something the editor is asked to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// A document command.
    Command(Command),
    /// Undo the last local change.
    Undo,
    /// Redo the last undone change.
    Redo,
    /// End the current undo group.
    BreakUndoGroup,
    /// Move the caret (`extend`: Shift held).
    MoveCaret {
        /// By what.
        granularity: Granularity,
        /// Which way.
        direction: Direction,
        /// Extend the selection instead of moving the caret.
        extend: bool,
    },
    /// Select the word at a position (double click).
    SelectWord(Position),
    /// Select the paragraph at a position (triple click).
    SelectParagraph(Position),
    /// Open a composition at the caret.
    CompositionStart,
    /// The composition's provisional text and caret (characters into it).
    CompositionUpdate(String, usize),
    /// Insert the composition's text.
    CompositionCommit,
    /// Drop the composition.
    CompositionCancel,
}

pub(super) const UNITS: [DeleteUnit; 2] = [DeleteUnit::Char, DeleteUnit::Word];
pub(super) const BREAKS: [BreakKind; 3] = [BreakKind::Line, BreakKind::Page, BreakKind::Column];
pub(super) const MARKS: [MarkKind; 6] = [
    MarkKind::Bold,
    MarkKind::Italic,
    MarkKind::Underline,
    MarkKind::Strike,
    MarkKind::Subscript,
    MarkKind::Superscript,
];
pub(super) const LISTS: [ListKind; 6] = ListKind::ALL;
pub(super) const GRANULARITIES: [Granularity; 6] = [
    Granularity::Char,
    Granularity::Word,
    Granularity::LineBoundary,
    Granularity::Line,
    Granularity::Paragraph,
    Granularity::Document,
];

fn code<T: PartialEq>(table: &[T], v: &T) -> u8 {
    u8::try_from(
        table
            .iter()
            .position(|t| t == v)
            .expect("every value is in its table"),
    )
    .expect("small tables")
}

pub(super) fn pick<T: Copy>(
    table: &[T],
    r: &mut Reader<'_>,
    what: &'static str,
) -> Result<T, DecodeError> {
    let tag = r.u8()?;
    table
        .get(usize::from(tag))
        .copied()
        .ok_or(DecodeError::BadTag { what, tag })
}

/// The binary form of a request.
#[must_use]
pub fn encode_request(req: &Request) -> Vec<u8> {
    let mut w = Writer::new();
    match req {
        Request::Command(c) => command(&mut w, c),
        Request::Undo => w.u8(0x40),
        Request::Redo => w.u8(0x41),
        Request::BreakUndoGroup => w.u8(0x42),
        Request::MoveCaret {
            granularity,
            direction,
            extend,
        } => {
            w.u8(0x43);
            w.u8(code(&GRANULARITIES, granularity));
            w.u8(u8::from(*direction == Direction::Forward));
            w.u8(u8::from(*extend));
        }
        Request::SelectWord(p) => {
            w.u8(0x44);
            w.position(*p);
        }
        Request::SelectParagraph(p) => {
            w.u8(0x45);
            w.position(*p);
        }
        Request::CompositionStart => w.u8(0x50),
        Request::CompositionUpdate(text, caret) => {
            w.u8(0x51);
            w.str(text);
            w.usize(*caret);
        }
        Request::CompositionCommit => w.u8(0x52),
        Request::CompositionCancel => w.u8(0x53),
    }
    w.finish()
}

fn command(w: &mut Writer, c: &Command) {
    match c {
        Command::InsertText(t) => {
            w.u8(0x01);
            w.str(t);
        }
        Command::DeleteBackward(u) => {
            w.u8(0x02);
            w.u8(code(&UNITS, u));
        }
        Command::DeleteForward(u) => {
            w.u8(0x03);
            w.u8(code(&UNITS, u));
        }
        Command::SplitParagraph => w.u8(0x04),
        Command::InsertBreak(k) => {
            w.u8(0x05);
            w.u8(code(&BREAKS, k));
        }
        Command::InsertTab => w.u8(0x06),
        Command::ToggleMark(m) => {
            w.u8(0x07);
            w.u8(code(&MARKS, m));
        }
        Command::SetRunProperty(p) => {
            w.u8(0x08);
            value!(w, p);
        }
        Command::SetParagraphProperty(p) => {
            w.u8(0x09);
            value!(w, p);
        }
        Command::SetParagraphStyle(s) => {
            w.u8(0x0A);
            w.str(s.as_str());
        }
        Command::SetNumbering { num_id, level } => {
            w.u8(0x0B);
            w.uvar(u64::from(num_id.0));
            w.u8(*level);
        }
        Command::SetNumberingKind { kind, level } => {
            w.u8(0x19);
            w.u8(code(&LISTS, kind));
            w.u8(*level);
        }
        Command::ClearNumbering => w.u8(0x0C),
        Command::Indent => w.u8(0x0D),
        Command::Outdent => w.u8(0x0E),
        Command::SetSectionProperty(p) => {
            w.u8(0x0F);
            value!(w, p);
        }
        Command::InsertTable { rows, cols } => {
            w.u8(0x10);
            w.uvar(u64::from(*rows));
            w.uvar(u64::from(*cols));
        }
        Command::InsertRowAbove => w.u8(0x11),
        Command::InsertRowBelow => w.u8(0x12),
        Command::InsertColumnLeft => w.u8(0x13),
        Command::InsertColumnRight => w.u8(0x14),
        Command::DeleteRow => w.u8(0x15),
        Command::DeleteColumn => w.u8(0x16),
        Command::DeleteTable => w.u8(0x17),
        Command::MergeCells => w.u8(0x18),
        Command::InsertImage { content_id, size } => {
            w.u8(if size.is_some() { 0x20 } else { 0x26 });
            w.str(content_id.as_str());
            if let Some((width, height)) = size {
                w.ivar(i64::from(width.0));
                w.ivar(i64::from(height.0));
            }
        }
        Command::InsertFootnote(Some(id)) => {
            w.u8(0x21);
            w.uvar(u64::from(id.0));
        }
        Command::InsertFootnote(None) => w.u8(0x25),
        Command::InsertLink(href) => {
            w.u8(0x22);
            w.str(href);
        }
        Command::RemoveLink => w.u8(0x23),
        Command::InsertComment(c) => {
            w.u8(0x24);
            value!(w, c);
        }
        Command::Replace { from, to, text } => {
            w.u8(0x30);
            w.position(*from);
            w.position(*to);
            w.str(text);
        }
        Command::SelectAll => w.u8(0x31),
        Command::SetSelection(s) => {
            w.u8(0x32);
            w.position(s.anchor);
            w.position(s.head);
        }
    }
}
