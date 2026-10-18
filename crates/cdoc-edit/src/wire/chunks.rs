//! A diff cut into updates, over the boundary.
//!
//! ```text
//! chunks := "CDK" 0x01 count:varint (update:bytes){count}
//! ```

use super::{Reader, Writer};
use cdoc_crdt::DecodeError;

const MAGIC: &[u8; 4] = b"CDK\x01";

/// The binary list of updates, in the order they are applied.
#[must_use]
pub fn encode_chunks(chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut w = Writer::new();
    w.raw(MAGIC);
    w.usize(chunks.len());
    for chunk in chunks {
        w.bytes(chunk);
    }
    w.finish()
}

/// Reads [`encode_chunks`]'s output.
pub fn decode_chunks(bytes: &[u8]) -> Result<Vec<Vec<u8>>, DecodeError> {
    let mut r = Reader::new(bytes);
    if r.take(4)? != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let n = r.usize()?;
    let chunks = (0..n)
        .map(|_| r.bytes().map(<[u8]>::to_vec))
        .collect::<Result<_, _>>()?;
    r.finish()?;
    Ok(chunks)
}
