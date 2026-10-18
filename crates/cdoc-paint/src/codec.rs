//! The primitive encodings: little-endian fixed integers, LEB128 varints, and zigzag for
//! signed values, so small magnitudes of either sign take one or two bytes.

use crate::DecodeError;

/// Signed to unsigned so that small magnitudes stay small: 0, -1, 1, -2 → 0, 1, 2, 3.
pub const fn zigzag(value: i32) -> u32 {
    #[expect(clippy::cast_sign_loss, reason = "zigzag is a bit reinterpretation")]
    let folded = ((value << 1) ^ (value >> 31)) as u32;
    folded
}

/// The inverse of [`zigzag`].
pub const fn unzigzag(value: u32) -> i32 {
    #[expect(
        clippy::cast_possible_wrap,
        reason = "zigzag is a bit reinterpretation"
    )]
    let unfolded = ((value >> 1) as i32) ^ -((value & 1) as i32);
    unfolded
}

/// Bytes being written.
#[derive(Debug)]
pub struct Writer {
    pub buf: Vec<u8>,
}

impl Writer {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
        }
    }

    pub fn u8(&mut self, value: u8) {
        self.buf.push(value);
    }

    pub fn u16(&mut self, value: u16) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    pub fn u32(&mut self, value: u32) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    /// An unsigned LEB128 varint: seven bits per byte, low bits first, high bit = more.
    pub fn uv(&mut self, mut value: u32) {
        while value >= 0x80 {
            #[expect(clippy::cast_possible_truncation, reason = "masked to seven bits")]
            self.buf.push((value as u8 & 0x7F) | 0x80);
            value >>= 7;
        }
        #[expect(clippy::cast_possible_truncation, reason = "below 0x80")]
        self.buf.push(value as u8);
    }

    /// A signed value, zigzagged then varint-coded.
    pub fn zz(&mut self, value: i32) {
        self.uv(zigzag(value));
    }

    /// Several signed values in order.
    pub fn zzs(&mut self, values: &[i32]) {
        for v in values {
            self.zz(*v);
        }
    }

    pub fn bytes(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Overwrites a `u32` written earlier at `at`.
    pub fn patch_u32(&mut self, at: usize, value: u32) {
        self.buf[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
}

/// A value a counted run in a payload is made of.
pub trait Decode: Sized {
    /// Reads one.
    fn decode(r: &mut Reader<'_>) -> Result<Self, DecodeError>;
}

/// Bytes being read; every read is bounds-checked and reports where it ran out.
#[derive(Debug, Clone)]
pub struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    /// Where `bytes` starts in the whole buffer, so errors name absolute offsets.
    base: usize,
}

impl<'a> Reader<'a> {
    pub const fn new(bytes: &'a [u8], base: usize) -> Self {
        Self {
            bytes,
            pos: 0,
            base,
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    pub fn take(&mut self, len: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .pos
            .checked_add(len)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(DecodeError::Truncated(self.base + self.bytes.len()))?;
        let out = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(out)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let mut out = [0; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    pub fn u8(&mut self) -> Result<u8, DecodeError> {
        let byte = *self
            .bytes
            .get(self.pos)
            .ok_or(DecodeError::Truncated(self.base + self.bytes.len()))?;
        self.pos += 1;
        Ok(byte)
    }

    pub fn u16(&mut self) -> Result<u16, DecodeError> {
        self.array().map(u16::from_le_bytes)
    }

    pub fn u32(&mut self) -> Result<u32, DecodeError> {
        self.array().map(u32::from_le_bytes)
    }

    pub fn i32(&mut self) -> Result<i32, DecodeError> {
        self.array().map(i32::from_le_bytes)
    }

    /// An unsigned LEB128 varint of at most five bytes that fits 32 bits. One and two bytes
    /// (every glyph id, cluster, offset and advance a page sets) are read inline.
    #[inline]
    pub fn uv(&mut self) -> Result<u32, DecodeError> {
        match *self.bytes.get(self.pos..).unwrap_or_default() {
            [lo, ..] if lo < 0x80 => {
                self.pos += 1;
                Ok(u32::from(lo))
            }
            [lo, hi, ..] if hi < 0x80 => {
                self.pos += 2;
                Ok(u32::from(lo & 0x7F) | u32::from(hi) << 7)
            }
            _ => self.uv_long(),
        }
    }

    #[cold]
    #[inline(never)]
    fn uv_long(&mut self) -> Result<u32, DecodeError> {
        let mut value: u32 = 0;
        for shift in [0, 7, 14, 21, 28] {
            let byte = self.u8()?;
            if shift == 28 && byte > 0x0F {
                return Err(DecodeError::Malformed("varint overflows 32 bits"));
            }
            value |= u32::from(byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(DecodeError::Malformed("varint longer than five bytes"))
    }

    #[inline]
    pub fn zz(&mut self) -> Result<i32, DecodeError> {
        self.uv().map(unzigzag)
    }

    /// A varint used as a count or index.
    pub fn len(&mut self) -> Result<usize, DecodeError> {
        self.uv()
            .and_then(|v| usize::try_from(v).map_err(|_| DecodeError::Malformed("count")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zigzag_keeps_small_magnitudes_small_and_round_trips_the_extremes() {
        assert_eq!(
            [0, -1, 1, -2, 2].map(zigzag),
            [0, 1, 2, 3, 4],
            "the zigzag order"
        );
        for v in [0, 1, -1, 63, -64, 64, i32::MAX, i32::MIN] {
            assert_eq!(unzigzag(zigzag(v)), v);
            let mut w = Writer::with_capacity(5);
            w.zz(v);
            assert_eq!(Reader::new(&w.buf, 0).zz(), Ok(v));
        }
    }

    #[test]
    fn varints_round_trip_either_side_of_each_byte_length() {
        for v in [
            0,
            1,
            127,
            128,
            129,
            300,
            16_383,
            16_384,
            2_097_151,
            2_097_152,
            u32::MAX,
        ] {
            let mut w = Writer::with_capacity(5);
            w.uv(v);
            w.uv(v);
            let mut r = Reader::new(&w.buf, 0);
            assert_eq!((r.uv(), r.uv()), (Ok(v), Ok(v)), "{v}");
            assert!(r.is_empty());
        }
    }

    #[test]
    fn varints_take_one_byte_below_128_and_refuse_overlong_input() {
        let mut w = Writer::with_capacity(8);
        w.uv(127);
        w.uv(128);
        assert_eq!(w.buf, [0x7F, 0x80, 0x01]);
        let overlong = [0xFF, 0xFF, 0xFF, 0xFF, 0x7F];
        assert!(Reader::new(&overlong, 0).uv().is_err());
        assert_eq!(Reader::new(&[0x80], 7).uv(), Err(DecodeError::Truncated(8)));
    }
}
