//! The byte layer every encoding in this crate is built on: unsigned LEB128 varints,
//! length-prefixed byte strings and a bounds-checked reader over borrowed input.

pub use crate::decode_error::DecodeError;

/// An append-only output buffer.
#[derive(Debug, Clone)]
pub(crate) struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub(crate) const fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// An empty buffer with room for `capacity` bytes.
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
        }
    }

    pub(crate) fn u8(&mut self, byte: u8) {
        self.buf.push(byte);
    }

    pub(crate) fn uvar(&mut self, mut value: u64) {
        while value >= 0x80 {
            // Truncation is the point: the low seven bits with the continuation flag.
            #[allow(clippy::cast_possible_truncation)]
            self.buf.push((value as u8) | 0x80);
            value >>= 7;
        }
        #[allow(clippy::cast_possible_truncation)]
        self.buf.push(value as u8);
    }

    /// A signed value zigzag-folded, then as a varint: small magnitudes either way are short.
    pub(crate) fn zvar(&mut self, value: i64) {
        // The fold is a bijection on 64 bits; the casts reinterpret, they never truncate.
        #[allow(clippy::cast_sign_loss)]
        self.uvar(((value << 1) ^ (value >> 63)) as u64);
    }

    pub(crate) fn usize(&mut self, value: usize) {
        self.uvar(value as u64);
    }

    pub(crate) fn raw(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    pub(crate) fn bytes(&mut self, bytes: &[u8]) {
        self.usize(bytes.len());
        self.raw(bytes);
    }

    pub(crate) fn str(&mut self, text: &str) {
        self.bytes(text.as_bytes());
    }

    pub(crate) fn len(&self) -> usize {
        self.buf.len()
    }

    /// Drops everything written after the first `len` bytes.
    pub(crate) fn truncate(&mut self, len: usize) {
        self.buf.truncate(len);
    }

    /// Inserts `bytes` at `at`, shifting what follows; used to backpatch a count.
    pub(crate) fn insert_at(&mut self, at: usize, bytes: &[u8]) {
        self.buf.splice(at..at, bytes.iter().copied());
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        self.buf
    }
}

/// A varint on its own, for backpatching.
pub(crate) fn varint(value: u64) -> Vec<u8> {
    let mut w = Writer::new();
    w.uvar(value);
    w.finish()
}

/// A cursor over borrowed input. Every read is bounds-checked; nothing is copied.
#[derive(Debug, Clone)]
pub(crate) struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub(crate) const fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    pub(crate) fn u8(&mut self) -> Result<u8, DecodeError> {
        let byte = *self.buf.get(self.pos).ok_or(DecodeError::UnexpectedEnd)?;
        self.pos += 1;
        Ok(byte)
    }

    pub(crate) fn peek(&self) -> Result<u8, DecodeError> {
        self.buf
            .get(self.pos)
            .copied()
            .ok_or(DecodeError::UnexpectedEnd)
    }

    /// A varint; one-byte values (most clocks deltas, lengths and tags) take the inlined path.
    #[inline]
    pub(crate) fn uvar(&mut self) -> Result<u64, DecodeError> {
        match self.buf.get(self.pos) {
            Some(&byte) if byte < 0x80 => {
                self.pos += 1;
                Ok(u64::from(byte))
            }
            _ => self.uvar_long(),
        }
    }

    fn uvar_long(&mut self) -> Result<u64, DecodeError> {
        let mut value = 0u64;
        let mut shift = 0u32;
        loop {
            let byte = self.u8()?;
            let low = u64::from(byte & 0x7f);
            if shift == 63 && low > 1 || shift > 63 {
                return Err(DecodeError::VarintOverflow);
            }
            value |= low << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
        }
    }

    /// Reads [`Writer::zvar`]'s output.
    pub(crate) fn zvar(&mut self) -> Result<i64, DecodeError> {
        let v = self.uvar()?;
        #[allow(clippy::cast_possible_wrap)]
        Ok(((v >> 1) as i64) ^ -((v & 1) as i64))
    }

    pub(crate) fn u32(&mut self, what: &'static str) -> Result<u32, DecodeError> {
        u32::try_from(self.uvar()?).map_err(|_| DecodeError::OutOfRange(what))
    }

    pub(crate) fn usize(&mut self, what: &'static str) -> Result<usize, DecodeError> {
        let n = usize::try_from(self.uvar()?).map_err(|_| DecodeError::OutOfRange(what))?;
        // A count can never exceed the bytes left: each counted thing takes at least one byte.
        if n > self.remaining() {
            return Err(DecodeError::OutOfRange(what));
        }
        Ok(n)
    }

    pub(crate) fn raw(&mut self, len: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .pos
            .checked_add(len)
            .ok_or(DecodeError::UnexpectedEnd)?;
        let bytes = self
            .buf
            .get(self.pos..end)
            .ok_or(DecodeError::UnexpectedEnd)?;
        self.pos = end;
        Ok(bytes)
    }

    pub(crate) fn bytes(&mut self) -> Result<&'a [u8], DecodeError> {
        let len = self.usize("byte string length")?;
        self.raw(len)
    }

    pub(crate) fn str(&mut self) -> Result<&'a str, DecodeError> {
        std::str::from_utf8(self.bytes()?).map_err(|_| DecodeError::InvalidUtf8)
    }

    /// How many bytes have been read.
    pub(crate) const fn position(&self) -> usize {
        self.pos
    }

    pub(crate) const fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    pub(crate) const fn finish(&self) -> Result<(), DecodeError> {
        match self.remaining() {
            0 => Ok(()),
            n => Err(DecodeError::Trailing(n)),
        }
    }
}
