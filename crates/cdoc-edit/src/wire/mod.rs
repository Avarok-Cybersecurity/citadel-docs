//! The editor's binary records (`docs/edit-commands.md`): requests in, results out. Integers
//! are unsigned LEB128 varints, signed ones zigzag-encoded first; strings are a varint length
//! then UTF-8; a model value (a property patch, a comment) is a varint length then its
//! `cdoc_crdt::codec` encoding. No floats, no JSON on the keystroke path.

/// A model value's codec bytes. Model values hold no floats, the only thing the codec refuses.
macro_rules! value {
    ($w:expr, $v:expr) => {
        $w.bytes(&cdoc_crdt::codec::to_bytes($v).expect("model values hold no floats"))
    };
}

/// Reads a model value's codec bytes, typed by where it goes.
macro_rules! read_value {
    ($r:expr) => {
        cdoc_crdt::codec::from_bytes($r.bytes()?)
            .map_err(|_| cdoc_crdt::DecodeError::Inconsistent("a model value did not decode"))?
    };
}

mod chunks;
mod formatting;
mod gc;
mod image;
mod peer;
mod rects;
mod request;
mod request_decode;
mod result;
mod story;
mod styles;
mod tables;

pub use chunks::{decode_chunks, encode_chunks};
pub use gc::{decode_collected, decode_known, encode_collected, encode_known};
pub use image::{
    decode_added_image, decode_image, decode_image_list, encode_added_image, encode_image,
    encode_image_list,
};
pub use peer::{decode_peer_view, encode_peer_view};
pub use rects::{decode_rects, encode_rects};
pub use request::{encode_request, Request};
pub use request_decode::decode_request;
pub use result::{decode_result, encode_result, Outcome};
pub use styles::{decode_styles, encode_styles};

use crate::position::Position;
use cdoc_crdt::{ClientId, DecodeError, Id};
use cdoc_layout::{Pt100, Rect};

/// Writes the format.
#[derive(Debug, Default)]
pub(crate) struct Writer(Vec<u8>);

impl Writer {
    pub(crate) fn new() -> Self {
        Self(Vec::new())
    }

    pub(crate) fn raw(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }

    pub(crate) fn u8(&mut self, b: u8) {
        self.0.push(b);
    }

    pub(crate) fn uvar(&mut self, mut v: u64) {
        while v >= 0x80 {
            self.0
                .push(u8::try_from(v & 0x7f).expect("seven bits") | 0x80);
            v >>= 7;
        }
        self.0.push(u8::try_from(v).expect("below 0x80"));
    }

    pub(crate) fn usize(&mut self, v: usize) {
        self.uvar(v as u64);
    }

    pub(crate) fn ivar(&mut self, v: i64) {
        self.uvar(((v << 1) ^ (v >> 63)).cast_unsigned());
    }

    pub(crate) fn bytes(&mut self, b: &[u8]) {
        self.usize(b.len());
        self.raw(b);
    }

    pub(crate) fn str(&mut self, s: &str) {
        self.bytes(s.as_bytes());
    }

    pub(crate) fn position(&mut self, p: Position) {
        self.uvar(p.after.client.0);
        self.uvar(p.after.clock);
    }

    pub(crate) fn rect(&mut self, r: Rect) {
        for v in [r.x, r.y, r.width, r.height] {
            self.ivar(i64::from(v));
        }
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        self.0
    }
}

/// Reads the format.
pub(crate) struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    pub(crate) const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, at: 0 }
    }

    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&e| e <= self.bytes.len())
            .ok_or(DecodeError::UnexpectedEnd)?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }

    pub(crate) fn u8(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    pub(crate) fn uvar(&mut self) -> Result<u64, DecodeError> {
        let mut v = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.u8()?;
            if shift == 63 && b > 1 {
                return Err(DecodeError::VarintOverflow);
            }
            v |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
        }
        Err(DecodeError::VarintOverflow)
    }

    pub(crate) fn usize(&mut self) -> Result<usize, DecodeError> {
        usize::try_from(self.uvar()?).map_err(|_| DecodeError::OutOfRange("length"))
    }

    pub(crate) fn ivar(&mut self) -> Result<i64, DecodeError> {
        let u = self.uvar()?;
        Ok((u >> 1).cast_signed() ^ -((u & 1).cast_signed()))
    }

    pub(crate) fn bytes(&mut self) -> Result<&'a [u8], DecodeError> {
        let n = self.usize()?;
        self.take(n)
    }

    pub(crate) fn str(&mut self) -> Result<&'a str, DecodeError> {
        std::str::from_utf8(self.bytes()?).map_err(|_| DecodeError::InvalidUtf8)
    }

    pub(crate) fn position(&mut self) -> Result<Position, DecodeError> {
        Ok(Position::after(Id::new(
            ClientId(self.uvar()?),
            self.uvar()?,
        )))
    }

    pub(crate) fn pt(&mut self) -> Result<Pt100, DecodeError> {
        Pt100::try_from(self.ivar()?).map_err(|_| DecodeError::OutOfRange("coordinate"))
    }

    pub(crate) fn finish(&self) -> Result<(), DecodeError> {
        match self.bytes.len() - self.at {
            0 => Ok(()),
            n => Err(DecodeError::Trailing(n)),
        }
    }
}

/// A position as bytes (what `hit_test` returns).
#[must_use]
pub fn encode_position(p: Position) -> Vec<u8> {
    let mut w = Writer::new();
    w.position(p);
    w.finish()
}

/// Reads [`encode_position`]'s output.
pub fn decode_position(bytes: &[u8]) -> Result<Position, DecodeError> {
    let mut r = Reader::new(bytes);
    let p = r.position()?;
    r.finish()?;
    Ok(p)
}
