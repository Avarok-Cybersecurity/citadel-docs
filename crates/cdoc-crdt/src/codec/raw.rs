//! Encoded values taken apart and put together without decoding what they hold: the entries of
//! a map, the elements of a sequence. Block properties merge field by field this way, so the
//! CRDT never needs to know the model types it stores.

use super::de::Decoder;
use super::{CodecError, MAP, SEQ, STR};
use crate::bytes::{DecodeError, Reader, Writer};
use serde::de::{Deserialize, IgnoredAny};

/// The first encoded value of `bytes` and what follows it.
fn split_value(bytes: &[u8]) -> Result<(&[u8], &[u8]), CodecError> {
    let mut de = Decoder::new(bytes);
    IgnoredAny::deserialize(&mut de)?;
    Ok(bytes.split_at(de.position()))
}

/// The count after a container's tag, and the bytes after the count.
fn open(bytes: &[u8], tag: u8) -> Result<(usize, &[u8]), CodecError> {
    let mut r = Reader::new(bytes);
    if r.u8()? != tag {
        return Err(DecodeError::Inconsistent("not the container expected").into());
    }
    let n = r.usize("container length")?;
    Ok((n, &bytes[r.position()..]))
}

/// The entries of an encoded map with string keys (an encoded struct), keys and values
/// borrowed, in encoded order.
pub(crate) fn map_entries(bytes: &[u8]) -> Result<Vec<(&str, &[u8])>, CodecError> {
    let (n, mut rest) = open(bytes, MAP)?;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let mut r = Reader::new(rest);
        if r.u8()? != STR {
            return Err(DecodeError::Inconsistent("a struct field name is not a string").into());
        }
        let key = r.str()?;
        let (value, after) = split_value(&rest[r.position()..])?;
        out.push((key, value));
        rest = after;
    }
    if !rest.is_empty() {
        return Err(DecodeError::Trailing(rest.len()).into());
    }
    Ok(out)
}

/// The elements of an encoded sequence of exactly `n` values, borrowed.
pub(crate) fn seq_items(bytes: &[u8], n: usize) -> Result<Vec<&[u8]>, CodecError> {
    let (count, mut rest) = open(bytes, SEQ)?;
    if count != n {
        return Err(DecodeError::Inconsistent("a tuple of the wrong length").into());
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let (value, after) = split_value(rest)?;
        out.push(value);
        rest = after;
    }
    if !rest.is_empty() {
        return Err(DecodeError::Trailing(rest.len()).into());
    }
    Ok(out)
}

/// An encoded map (struct) of these entries.
pub(crate) fn write_map<'a>(entries: &[(&'a str, &'a [u8])]) -> Vec<u8> {
    let mut w = Writer::new();
    w.u8(MAP);
    w.usize(entries.len());
    for (key, value) in entries {
        w.u8(STR);
        w.str(key);
        w.raw(value);
    }
    w.finish()
}

/// An encoded sequence (tuple) of these elements.
pub(crate) fn write_seq(items: &[&[u8]]) -> Vec<u8> {
    let mut w = Writer::new();
    w.u8(SEQ);
    w.usize(items.len());
    for item in items {
        w.raw(item);
    }
    w.finish()
}
