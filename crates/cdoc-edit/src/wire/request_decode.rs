//! Reading request records.

use super::request::{pick, Request, BREAKS, GRANULARITIES, MARKS, UNITS};
use super::Reader;
use crate::commands::Command;
use crate::position::{Direction, Selection};
use cdoc_crdt::DecodeError;
use cdoc_model::{ContentId, FootnoteId, Length, NumId, StyleId};

fn u32_of(r: &mut Reader<'_>, what: &'static str) -> Result<u32, DecodeError> {
    u32::try_from(r.uvar()?).map_err(|_| DecodeError::OutOfRange(what))
}

fn length(r: &mut Reader<'_>) -> Result<Length, DecodeError> {
    Ok(Length(r.pt()?))
}

/// Reads [`encode_request`](super::encode_request)'s output.
pub fn decode_request(bytes: &[u8]) -> Result<Request, DecodeError> {
    let mut r = Reader::new(bytes);
    let op = r.u8()?;
    let c = |c: Command| Request::Command(c);
    let req = match op {
        0x01 => c(Command::InsertText(r.str()?.to_owned())),
        0x02 => c(Command::DeleteBackward(pick(
            &UNITS,
            &mut r,
            "delete unit",
        )?)),
        0x03 => c(Command::DeleteForward(pick(&UNITS, &mut r, "delete unit")?)),
        0x04 => c(Command::SplitParagraph),
        0x05 => c(Command::InsertBreak(pick(&BREAKS, &mut r, "break kind")?)),
        0x06 => c(Command::InsertTab),
        0x07 => c(Command::ToggleMark(pick(&MARKS, &mut r, "mark kind")?)),
        0x08 => c(Command::SetRunProperty(read_value!(r))),
        0x09 => c(Command::SetParagraphProperty(read_value!(r))),
        0x0A => c(Command::SetParagraphStyle(StyleId::new(r.str()?))),
        0x0B => c(Command::SetNumbering {
            num_id: NumId(u32_of(&mut r, "num id")?),
            level: r.u8()?,
        }),
        0x0C => c(Command::ClearNumbering),
        0x0D => c(Command::Indent),
        0x0E => c(Command::Outdent),
        0x0F => c(Command::SetSectionProperty(read_value!(r))),
        0x10 => c(Command::InsertTable {
            rows: u32_of(&mut r, "rows")?,
            cols: u32_of(&mut r, "cols")?,
        }),
        0x11 => c(Command::InsertRowAbove),
        0x12 => c(Command::InsertRowBelow),
        0x13 => c(Command::InsertColumnLeft),
        0x14 => c(Command::InsertColumnRight),
        0x15 => c(Command::DeleteRow),
        0x16 => c(Command::DeleteColumn),
        0x17 => c(Command::DeleteTable),
        0x18 => c(Command::MergeCells),
        0x20 => c(Command::InsertImage {
            content_id: ContentId::new(r.str()?),
            width: length(&mut r)?,
            height: length(&mut r)?,
        }),
        0x21 => c(Command::InsertFootnote(FootnoteId(u32_of(
            &mut r,
            "footnote id",
        )?))),
        0x22 => c(Command::InsertLink(r.str()?.to_owned())),
        0x23 => c(Command::RemoveLink),
        0x24 => c(Command::InsertComment(read_value!(r))),
        0x30 => c(Command::Replace {
            from: r.position()?,
            to: r.position()?,
            text: r.str()?.to_owned(),
        }),
        0x31 => c(Command::SelectAll),
        0x32 => c(Command::SetSelection(Selection {
            anchor: r.position()?,
            head: r.position()?,
        })),
        0x40 => Request::Undo,
        0x41 => Request::Redo,
        0x42 => Request::BreakUndoGroup,
        0x43 => Request::MoveCaret {
            granularity: pick(&GRANULARITIES, &mut r, "granularity")?,
            direction: if r.u8()? == 0 {
                Direction::Backward
            } else {
                Direction::Forward
            },
            extend: r.u8()? != 0,
        },
        0x44 => Request::SelectWord(r.position()?),
        0x45 => Request::SelectParagraph(r.position()?),
        0x50 => Request::CompositionStart,
        0x51 => Request::CompositionUpdate(r.str()?.to_owned(), r.usize()?),
        0x52 => Request::CompositionCommit,
        0x53 => Request::CompositionCancel,
        tag => {
            return Err(DecodeError::BadTag {
                what: "request",
                tag,
            })
        }
    };
    r.finish()?;
    Ok(req)
}
