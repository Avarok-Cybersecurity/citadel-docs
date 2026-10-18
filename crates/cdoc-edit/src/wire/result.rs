//! The result record: what the UI needs after any request or remote update.
//!
//! ```text
//! result := "CDE" 0x01 update:bytes count:varint page:varint{count} page_count:varint
//!           anchor:position head:position caret:(0 | 1 page:varint rect)
//!           count:varint (page:varint rect){count}
//! rect   := x:zigzag y:zigzag width:zigzag height:zigzag      (hundredths of a point)
//! ```

use super::{Reader, Writer};
use crate::position::Selection;
use cdoc_crdt::DecodeError;
use cdoc_layout::Rect;

const MAGIC: &[u8; 4] = b"CDE\x01";

/// A result as the record carries it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    /// The update for the relay; empty when nothing changed (always empty for remote updates).
    pub update: Vec<u8>,
    /// Pages to repaint.
    pub dirty_pages: Vec<usize>,
    /// Pages in the document.
    pub page_count: usize,
    /// The selection.
    pub selection: Selection,
    /// The caret, with its page.
    pub caret: Option<(usize, Rect)>,
    /// The selection highlight, per line, with pages.
    pub rects: Vec<(usize, Rect)>,
}

/// The binary record.
#[must_use]
pub fn encode_result(o: &Outcome) -> Vec<u8> {
    let mut w = Writer::new();
    w.raw(MAGIC);
    w.bytes(&o.update);
    w.usize(o.dirty_pages.len());
    for &p in &o.dirty_pages {
        w.usize(p);
    }
    w.usize(o.page_count);
    w.position(o.selection.anchor);
    w.position(o.selection.head);
    write_caret(&mut w, o.caret);
    write_rects(&mut w, &o.rects);
    w.finish()
}

pub(super) fn write_caret(w: &mut Writer, caret: Option<(usize, Rect)>) {
    match caret {
        Some((page, rect)) => {
            w.u8(1);
            w.usize(page);
            w.rect(rect);
        }
        None => w.u8(0),
    }
}

pub(super) fn write_rects(w: &mut Writer, rects: &[(usize, Rect)]) {
    w.usize(rects.len());
    for &(page, rect) in rects {
        w.usize(page);
        w.rect(rect);
    }
}

pub(super) fn read_caret(r: &mut Reader<'_>) -> Result<Option<(usize, Rect)>, DecodeError> {
    match r.u8()? {
        0 => Ok(None),
        1 => Ok(Some((r.usize()?, rect(r)?))),
        tag => Err(DecodeError::BadTag { what: "caret", tag }),
    }
}

pub(super) fn read_rects(r: &mut Reader<'_>) -> Result<Vec<(usize, Rect)>, DecodeError> {
    let n = r.usize()?;
    (0..n).map(|_| Ok((r.usize()?, rect(r)?))).collect()
}

fn rect(r: &mut Reader<'_>) -> Result<Rect, DecodeError> {
    Ok(Rect {
        x: r.pt()?,
        y: r.pt()?,
        width: r.pt()?,
        height: r.pt()?,
    })
}

/// Reads [`encode_result`]'s output.
pub fn decode_result(bytes: &[u8]) -> Result<Outcome, DecodeError> {
    let mut r = Reader::new(bytes);
    if r.take(4)? != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let update = r.bytes()?.to_vec();
    let n = r.usize()?;
    let dirty_pages = (0..n).map(|_| r.usize()).collect::<Result<_, _>>()?;
    let page_count = r.usize()?;
    let selection = Selection {
        anchor: r.position()?,
        head: r.position()?,
    };
    let caret = read_caret(&mut r)?;
    let rects = read_rects(&mut r)?;
    r.finish()?;
    Ok(Outcome {
        update,
        dirty_pages,
        page_count,
        selection,
        caret,
        rects,
    })
}
