//! A compact, self-describing binary encoding of `serde` values, used for the model values an
//! atom, a mark or an attribute carries (paragraph properties, a link, the style sheet, ...).
//!
//! Self-describing because the model's serde attributes need it: internally tagged enums
//! (`#[serde(tag = "type")]`) and `skip_serializing_if` fields cannot be read back by a format
//! that relies on the reader knowing the shape. The data model is JSON's, in bytes:
//!
//! | tag | value | body |
//! |---|---|---|
//! | 0 | null (`None`, unit) | |
//! | 1, 2 | `false`, `true` | |
//! | 3 | unsigned integer | varint |
//! | 4 | negative integer `-1 - n` | varint `n` |
//! | 5 | string | varint length, UTF-8 |
//! | 6 | byte string | varint length, bytes |
//! | 7 | sequence | varint count, values |
//! | 8 | map | varint count, key/value pairs |
//!
//! Enums are spelled as JSON spells them: a unit variant is its name, any other variant a
//! one-entry map from name to content. **There is no float tag**: serialising an `f32` or
//! `f64` is [`CodecError::Float`], which is how "no floats cross a serialisation boundary" is
//! enforced rather than hoped for.

mod compound;
mod de;
mod ser;

use crate::bytes::DecodeError;
use serde::de::DeserializeOwned;
use serde::Serialize;

pub(crate) const NULL: u8 = 0;
pub(crate) const FALSE: u8 = 1;
pub(crate) const TRUE: u8 = 2;
pub(crate) const UINT: u8 = 3;
pub(crate) const NINT: u8 = 4;
pub(crate) const STR: u8 = 5;
pub(crate) const BYTES: u8 = 6;
pub(crate) const SEQ: u8 = 7;
pub(crate) const MAP: u8 = 8;

/// A value this codec cannot write or read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CodecError {
    /// A floating-point number: forbidden on every boundary.
    #[error("floating-point values are not encodable")]
    Float,
    /// Malformed bytes.
    #[error(transparent)]
    Decode(#[from] DecodeError),
    /// `serde` reported a shape mismatch.
    #[error("{0}")]
    Message(String),
}

impl serde::ser::Error for CodecError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Self::Message(msg.to_string())
    }
}

impl serde::de::Error for CodecError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        Self::Message(msg.to_string())
    }
}

/// Encodes a value.
pub fn to_bytes<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, CodecError> {
    let mut out = ser::Encoder::new();
    value.serialize(&mut out)?;
    Ok(out.finish())
}

/// Decodes a value; the whole input must be consumed.
pub fn from_bytes<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, CodecError> {
    let mut input = de::Decoder::new(bytes);
    let value = T::deserialize(&mut input)?;
    input.finish()?;
    Ok(value)
}
