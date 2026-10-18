//! Tombstone collection over the boundary: the peers' state vectors in, the counts out.
//!
//! ```text
//! known     := count:varint (state_vector:bytes){count}
//! collected := "CDG" 0x02 units:varint deletes:varint pending:varint
//! ```

use super::{Reader, Writer};
use cdoc_crdt::{Collected, DecodeError, StateVector};

/// 2: adds the buffered operations collection dropped.
const MAGIC: &[u8; 4] = b"CDG\x02";

/// The binary list of the peers' state vectors.
#[must_use]
pub fn encode_known(known: &[StateVector]) -> Vec<u8> {
    let mut w = Writer::new();
    w.usize(known.len());
    for sv in known {
        w.bytes(&sv.encode());
    }
    w.finish()
}

/// Reads [`encode_known`]'s output.
pub fn decode_known(bytes: &[u8]) -> Result<Vec<StateVector>, DecodeError> {
    let mut r = Reader::new(bytes);
    let n = r.usize()?;
    let known = (0..n)
        .map(|_| StateVector::decode(r.bytes()?))
        .collect::<Result<_, _>>()?;
    r.finish()?;
    Ok(known)
}

/// The binary form of what collection removed.
#[must_use]
pub fn encode_collected(c: &Collected) -> Vec<u8> {
    let mut w = Writer::new();
    w.raw(MAGIC);
    w.uvar(c.units);
    w.uvar(c.deletes);
    w.uvar(c.pending);
    w.finish()
}

/// Reads [`encode_collected`]'s output.
pub fn decode_collected(bytes: &[u8]) -> Result<Collected, DecodeError> {
    let mut r = Reader::new(bytes);
    if r.take(4)? != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let c = Collected {
        units: r.uvar()?,
        deletes: r.uvar()?,
        pending: r.uvar()?,
    };
    r.finish()?;
    Ok(c)
}
