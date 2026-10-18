//! The peer-view record: a peer's caret and selection as rectangles to draw.
//!
//! ```text
//! peer_view := "CDP" 0x01 client:varint caret:(0 | 1 page:varint rect)
//!              count:varint (page:varint rect){count}
//! ```

use super::result::{read_caret, read_rects, write_caret, write_rects};
use super::{Reader, Writer};
use crate::editor::PeerView;
use cdoc_crdt::{ClientId, DecodeError};

const MAGIC: &[u8; 4] = b"CDP\x01";

/// The binary record.
#[must_use]
pub fn encode_peer_view(v: &PeerView) -> Vec<u8> {
    let mut w = Writer::new();
    w.raw(MAGIC);
    w.uvar(v.client.0);
    write_caret(&mut w, v.caret);
    write_rects(&mut w, &v.rects);
    w.finish()
}

/// Reads [`encode_peer_view`]'s output.
pub fn decode_peer_view(bytes: &[u8]) -> Result<PeerView, DecodeError> {
    let mut r = Reader::new(bytes);
    if r.take(4)? != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let client = ClientId(r.uvar()?);
    let caret = read_caret(&mut r)?;
    let rects = read_rects(&mut r)?;
    r.finish()?;
    Ok(PeerView {
        client,
        caret,
        rects,
    })
}
