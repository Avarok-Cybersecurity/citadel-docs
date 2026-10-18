//! The result record: what the UI needs after any request or remote update.
//!
//! ```text
//! result := "CDE" 0x05 update:bytes count:varint page:varint{count} page_count:varint
//!           anchor:position head:position caret:(0 | 1 page:varint rect)
//!           count:varint (page:varint rect){count} formatting created section
//!           selection_text remote
//! rect   := x:zigzag y:zigzag width:zigzag height:zigzag      (hundredths of a point)
//! created := count:varint (kind:u8 id:varint){count}          kind 0 footnote, 1 numbering
//! section := page:value                                       the caret's section setup
//! selection_text := complete:u8 text:str     the selection's text; complete 0 and an empty
//!                                            str past MAX_SELECTION_TEXT_BYTES
//! ```
//!
//! Version 2 is version 1 with the formatting section (`formatting.rs`) appended; version 3
//! appends what the request made under ids the editor picked; version 4 appends the page
//! setup of the caret's section (and the selection's text); version 5 appends what a peer's update left waiting (`remote.rs`).

use super::remote::Remote;
use super::{Reader, Writer};
use crate::caret::CaretFormatting;
use crate::commands::Created;
use crate::position::Selection;
use cdoc_crdt::DecodeError;
use cdoc_layout::Rect;
use cdoc_model::{FootnoteId, NumId, PageSetup};

const MAGIC: &[u8; 3] = b"CDE";
const VERSION: u8 = 5;

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
    /// Formatting where the selection starts.
    pub formatting: CaretFormatting,
    /// What the request made under ids the editor picked.
    pub created: Vec<Created>,
    /// The page setup of the section the caret is in.
    pub section: PageSetup,
    /// The selection's plain text, `None` past [`crate::MAX_SELECTION_TEXT_BYTES`].
    pub selection_text: Option<String>,
    /// What applying a peer's update left waiting; `None` for a local request.
    pub remote: Option<Remote>,
}

/// The binary record.
#[must_use]
pub fn encode_result(o: &Outcome) -> Vec<u8> {
    let mut w = Writer::new();
    w.raw(MAGIC);
    w.u8(VERSION);
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
    super::formatting::write(&mut w, &o.formatting);
    w.usize(o.created.len());
    for c in &o.created {
        let (kind, id) = match c {
            Created::Footnote(id) => (0, id.0),
            Created::Numbering(id) => (1, id.0),
        };
        w.u8(kind);
        w.uvar(u64::from(id));
    }
    value!(w, &o.section);
    w.u8(u8::from(o.selection_text.is_some()));
    w.str(o.selection_text.as_deref().unwrap_or(""));
    super::remote::write(&mut w, o.remote);
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

pub(super) fn rect(r: &mut Reader<'_>) -> Result<Rect, DecodeError> {
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
    if r.take(3)? != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let found = r.u8()?;
    if found != VERSION {
        return Err(DecodeError::RecordVersion {
            record: "result",
            found,
            supported: VERSION,
        });
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
    let formatting = super::formatting::read(&mut r)?;
    let n = r.usize()?;
    let created = (0..n)
        .map(|_| {
            let kind = r.u8()?;
            let id = u32::try_from(r.uvar()?).map_err(|_| DecodeError::OutOfRange("created id"))?;
            match kind {
                0 => Ok(Created::Footnote(FootnoteId(id))),
                1 => Ok(Created::Numbering(NumId(id))),
                tag => Err(DecodeError::BadTag {
                    what: "created",
                    tag,
                }),
            }
        })
        .collect::<Result<_, _>>()?;
    let section = read_value!(r);
    let complete = r.u8()?;
    let text = r.str()?.to_owned();
    let selection_text = match complete {
        1 => Some(text),
        0 if text.is_empty() => None,
        0 => {
            return Err(DecodeError::Inconsistent(
                "text with an incomplete selection",
            ))
        }
        tag => {
            return Err(DecodeError::BadTag {
                what: "complete",
                tag,
            })
        }
    };
    let remote = super::remote::read(&mut r)?;
    r.finish()?;
    Ok(Outcome {
        update,
        dirty_pages,
        page_count,
        selection,
        caret,
        rects,
        formatting,
        created,
        section,
        selection_text,
        remote,
    })
}
