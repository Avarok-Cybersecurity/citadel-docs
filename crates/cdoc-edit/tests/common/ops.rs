//! Random editing operations, shared by the property tests.

use super::at;
use cdoc_edit::{Command, DeleteUnit, EditError, Editor, MarkKind, Selection};
use cdoc_model::{
    Alignment, BreakKind, Comment, CommentId, ContentId, FontSize, FootnoteId, Length, NumId,
    ParagraphPatch, RunPatch, StyleId,
};
use proptest::prelude::*;

/// One random operation.
#[derive(Debug, Clone)]
pub enum Op {
    /// Command `0`, with two random arguments.
    Cmd(u8, u16, u16),
    Select(u16, u16),
    Undo,
    Redo,
}

pub fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        6 => (0u8..34, any::<u16>(), any::<u16>()).prop_map(|(c, a, b)| Op::Cmd(c, a, b)),
        3 => (any::<u16>(), any::<u16>()).prop_map(|(a, b)| Op::Select(a, b)),
        1 => Just(Op::Undo),
        1 => Just(Op::Redo),
    ]
}

const TEXTS: [&str; 6] = ["a", "word", " ", "two words", "line\nbreak", "tab\there"];

pub fn command(e: &Editor<'_>, c: u8, a: u16, b: u16, serial: usize) -> Command {
    let len = e.doc().len().max(1);
    let gap = |v: u16| at(e, 1 + usize::from(v) % len);
    match c {
        0..=5 => Command::InsertText(TEXTS[usize::from(c)].to_owned()),
        6 => Command::DeleteBackward(DeleteUnit::Char),
        7 => Command::DeleteBackward(DeleteUnit::Word),
        8 => Command::DeleteForward(DeleteUnit::Char),
        9 => Command::DeleteForward(DeleteUnit::Word),
        10 => Command::SplitParagraph,
        11 => Command::InsertBreak(BreakKind::Line),
        12 => Command::InsertTab,
        13 => Command::ToggleMark(
            [
                MarkKind::Bold,
                MarkKind::Italic,
                MarkKind::Underline,
                MarkKind::Strike,
                MarkKind::Subscript,
                MarkKind::Superscript,
            ][usize::from(a) % 6],
        ),
        14 => Command::SetRunProperty(RunPatch {
            size: Some(Some(FontSize(20 + a % 20))),
            ..RunPatch::default()
        }),
        15 => Command::SetParagraphProperty(ParagraphPatch {
            alignment: Some(Some(Alignment::Center)),
            ..ParagraphPatch::default()
        }),
        16 => Command::SetParagraphStyle(StyleId::new(if a.is_multiple_of(2) {
            "Heading1"
        } else {
            "Normal"
        })),
        17 => Command::SetNumbering {
            num_id: NumId(1),
            level: u8::try_from(a % 3).unwrap_or(0),
        },
        18 => Command::ClearNumbering,
        19 => {
            if a.is_multiple_of(2) {
                Command::Indent
            } else {
                Command::Outdent
            }
        }
        20 => Command::InsertTable {
            rows: 1 + u32::from(a % 3),
            cols: 1 + u32::from(b % 3),
        },
        21 => [Command::InsertRowAbove, Command::InsertRowBelow][usize::from(a) % 2].clone(),
        22 => [Command::InsertColumnLeft, Command::InsertColumnRight][usize::from(a) % 2].clone(),
        23 => Command::DeleteRow,
        24 => Command::DeleteColumn,
        25 => Command::DeleteTable,
        26 => Command::MergeCells,
        27 => Command::InsertImage {
            content_id: ContentId::new("img"),
            size: Some((Length(7200), Length(3600))),
        },
        28 => Command::InsertFootnote(Some(FootnoteId(1000 + u32::try_from(serial).unwrap_or(0)))),
        29 => Command::InsertLink("https://example.test".to_owned()),
        30 => Command::RemoveLink,
        31 => Command::InsertComment(Comment {
            id: CommentId::new(format!("c{serial}")),
            author: "A".to_owned(),
            date: "2026-10-17T00:00:00Z".to_owned(),
            text: "note".to_owned(),
            resolved: false,
            parent: None,
        }),
        32 => Command::Replace {
            from: gap(a),
            to: gap(b),
            text: "R".to_owned(),
        },
        _ => Command::SelectAll,
    }
}

pub fn refused(e: &EditError) -> bool {
    matches!(
        e,
        EditError::NotApplicable(_) | EditError::InvalidArgument(_)
    )
}

/// Runs `op` (the `serial`-th): its update when it changed the document. Panics when it
/// failed for a reason other than not applying where it was tried.
pub fn apply(e: &mut Editor<'_>, op: &Op, serial: usize) -> Option<Vec<u8>> {
    run(e, op, serial, refused)
}

/// [`apply`] on one of several replicas: concurrent structural edits can leave a sequence
/// outside the grammar, which the editor shows repaired and refuses structural commands on
/// (`outline.rs`), so `Irregular` is a refusal there too.
pub fn apply_on_peer(e: &mut Editor<'_>, op: &Op, serial: usize) -> Option<Vec<u8>> {
    run(e, op, serial, |err| {
        refused(err) || matches!(err, EditError::Irregular)
    })
}

fn run(
    e: &mut Editor<'_>,
    op: &Op,
    serial: usize,
    refusal: impl Fn(&EditError) -> bool,
) -> Option<Vec<u8>> {
    let len = e.doc().len().max(1);
    let result = match op {
        Op::Cmd(c, a, b) => {
            let cmd = command(e, *c, *a, *b, serial);
            e.apply(cmd).map(Some)
        }
        Op::Select(a, b) => {
            let anchor = at(e, 1 + usize::from(*a) % len);
            let head = at(e, 1 + usize::from(*b) % len);
            e.apply(Command::SetSelection(Selection { anchor, head }))
                .map(Some)
        }
        Op::Undo => e.undo(),
        Op::Redo => e.redo(),
    };
    match result {
        Ok(applied) => applied.map(|a| a.update).filter(|u| !u.is_empty()),
        Err(err) => {
            assert!(refusal(&err), "{op:?} failed: {err:?}");
            None
        }
    }
}
