//! Wire form of a delete operation: the clock ranges of the units it deletes, sorted by client
//! and clock, disjoint and not touching.

use crate::bytes::{DecodeError, Reader, Writer};
use crate::id::ClientId;
use crate::update::ops::DelRange;

/// `k:varint (client:varint start:varint len:varint){k}`.
pub(crate) fn write_delete(w: &mut Writer, ranges: &[DelRange]) {
    w.usize(ranges.len());
    for r in ranges {
        w.uvar(r.client.0);
        w.uvar(r.start);
        w.uvar(r.end - r.start);
    }
}

pub(crate) fn read_delete(r: &mut Reader<'_>) -> Result<Vec<DelRange>, DecodeError> {
    let n = r.usize("delete range count")?;
    if n == 0 {
        return Err(DecodeError::Inconsistent("a delete of nothing"));
    }
    let mut out: Vec<DelRange> = Vec::with_capacity(n);
    for _ in 0..n {
        let client = ClientId(r.uvar()?);
        let start = r.uvar()?;
        let len = r.uvar()?;
        let end = start
            .checked_add(len)
            .ok_or(DecodeError::OutOfRange("clock"))?;
        push_canonical(&mut out, DelRange { client, start, end })?;
    }
    Ok(out)
}

/// Appends `r` to a delete's ranges, refusing an empty one or one that is not after the last
/// (sorted, disjoint, not touching).
pub(crate) fn push_canonical(out: &mut Vec<DelRange>, r: DelRange) -> Result<(), DecodeError> {
    let canonical = r.end > r.start
        && out
            .last()
            .is_none_or(|p| p.client < r.client || (p.client == r.client && p.end < r.start));
    if !canonical {
        return Err(DecodeError::Inconsistent("delete ranges not canonical"));
    }
    out.push(r);
    Ok(())
}
