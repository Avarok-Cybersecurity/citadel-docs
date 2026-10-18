//! The selection's rectangles on one page, for a page scrolled into view (results carry them
//! for the viewport and the dirty pages only).
//!
//! ```text
//! rects := "CDR" 0x01 count:varint rect{count}
//! ```

use super::{Reader, Writer};
use cdoc_crdt::DecodeError;
use cdoc_layout::Rect;

const MAGIC: &[u8; 4] = b"CDR\x01";

/// The binary form of one page's selection rectangles.
#[must_use]
pub fn encode_rects(rects: &[Rect]) -> Vec<u8> {
    let mut w = Writer::new();
    w.raw(MAGIC);
    w.usize(rects.len());
    for &r in rects {
        w.rect(r);
    }
    w.finish()
}

/// Reads [`encode_rects`]'s output.
pub fn decode_rects(bytes: &[u8]) -> Result<Vec<Rect>, DecodeError> {
    let mut r = Reader::new(bytes);
    if r.take(4)? != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let n = r.usize()?;
    let out = (0..n)
        .map(|_| super::result::rect(&mut r))
        .collect::<Result<_, _>>()?;
    r.finish()?;
    Ok(out)
}
