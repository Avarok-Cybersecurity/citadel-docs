//! The byte layer every encoding in this crate is built on: unsigned LEB128 varints,
//! length-prefixed byte strings and a bounds-checked reader over borrowed input.

/// Bytes that are not a well-formed encoding.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    /// The input ended inside a value.
    #[error("unexpected end of input")]
    UnexpectedEnd,
    /// A varint longer than 64 bits.
    #[error("varint overflows 64 bits")]
    VarintOverflow,
    /// Text that is not UTF-8.
    #[error("invalid UTF-8")]
    InvalidUtf8,
    /// An unknown tag byte.
    #[error("unknown {what} tag {tag}")]
    BadTag {
        /// What was being read.
        what: &'static str,
        /// The tag found.
        tag: u8,
    },
    /// A record of another version than the one this build reads (the editor's records).
    #[error("{record} record version {found}; this build reads only version {supported}")]
    RecordVersion {
        /// Which record.
        record: &'static str,
        /// The version found.
        found: u8,
        /// The one version this build reads.
        supported: u8,
    },
    /// The input does not start with this crate's magic.
    #[error("not a cdoc-crdt encoding (bad magic)")]
    BadMagic,
    /// An encoding of another format version. Version 1 shipped deletions as an unclocked set
    /// that cannot be turned into version 2's clocked delete operations, so it is refused,
    /// never guessed at: re-encode it with the version that wrote it.
    #[error("cdoc-crdt format version {found}, this build reads only version {supported}")]
    Version {
        /// The version found.
        found: u8,
        /// The one version this build reads.
        supported: u8,
    },
    /// The update needs history this replica or its sender has garbage-collected: the receiver
    /// is behind the sender's GC frontier, or the operation names a collected tombstone. The
    /// replica is unchanged; it must be reloaded from a snapshot.
    #[error("the update needs history compacted away by garbage collection")]
    Compacted,
    /// A value that does not fit the field it is read into.
    #[error("{0} out of range")]
    OutOfRange(&'static str),
    /// Values that are individually well formed but contradict each other.
    #[error("inconsistent encoding: {0}")]
    Inconsistent(&'static str),
    /// Bytes left over after the value.
    #[error("{0} trailing bytes")]
    Trailing(usize),
}

/// An append-only output buffer.
#[derive(Debug, Clone)]
pub(crate) struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    pub(crate) const fn new() -> Self {
        Self { buf: Vec::new() }
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
