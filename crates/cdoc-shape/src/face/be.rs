//! Bounds-checked big-endian reads for the few table parts `ttf-parser` does not expose.

pub(crate) fn u16_at(data: &[u8], offset: usize) -> Option<u16> {
    let bytes = data.get(offset..offset.checked_add(2)?)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

pub(crate) fn i16_at(data: &[u8], offset: usize) -> Option<i16> {
    u16_at(data, offset).map(|v| i16::from_be_bytes(v.to_be_bytes()))
}

/// Whether `glyph` is in an OpenType coverage table, and at which index.
pub(crate) fn coverage_index(coverage: &[u8], glyph: u16) -> Option<u16> {
    let count = usize::from(u16_at(coverage, 2)?);
    match u16_at(coverage, 0)? {
        1 => {
            let (mut lo, mut hi) = (0, count);
            while lo < hi {
                let mid = usize::midpoint(lo, hi);
                let g = u16_at(coverage, 4 + mid * 2)?;
                match g.cmp(&glyph) {
                    std::cmp::Ordering::Less => lo = mid + 1,
                    std::cmp::Ordering::Greater => hi = mid,
                    std::cmp::Ordering::Equal => return u16::try_from(mid).ok(),
                }
            }
            None
        }
        2 => {
            let (mut lo, mut hi) = (0, count);
            while lo < hi {
                let mid = usize::midpoint(lo, hi);
                let at = 4 + mid * 6;
                let (start, end) = (u16_at(coverage, at)?, u16_at(coverage, at + 2)?);
                if glyph < start {
                    hi = mid;
                } else if glyph > end {
                    lo = mid + 1;
                } else {
                    return u16_at(coverage, at + 4)?.checked_add(glyph - start);
                }
            }
            None
        }
        _ => None,
    }
}
