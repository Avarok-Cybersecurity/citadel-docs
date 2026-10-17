//! The reading half of the codec: a `serde::Deserializer` over borrowed bytes. Strings and byte
//! strings are handed to the visitor borrowed, so decoding copies only what the target type owns.

use super::{CodecError, BYTES, FALSE, MAP, NINT, NULL, SEQ, STR, TRUE, UINT};
use crate::bytes::{DecodeError, Reader};
use serde::de::{
    self, DeserializeSeed, Deserializer, EnumAccess, IntoDeserializer, MapAccess, SeqAccess,
    VariantAccess, Visitor,
};

/// Containers nested deeper than this are rejected rather than risking the stack.
const MAX_DEPTH: u32 = 128;

pub(crate) struct Decoder<'de> {
    r: Reader<'de>,
    depth: u32,
}

impl<'de> Decoder<'de> {
    pub(crate) const fn new(bytes: &'de [u8]) -> Self {
        Self {
            r: Reader::new(bytes),
            depth: 0,
        }
    }

    pub(crate) fn finish(&self) -> Result<(), CodecError> {
        Ok(self.r.finish()?)
    }

    fn container<V, F>(&mut self, visit: F) -> Result<V, CodecError>
    where
        F: FnOnce(&mut Self, usize) -> Result<(V, usize), CodecError>,
    {
        let count = self.r.usize("container length")?;
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(DecodeError::OutOfRange("nesting depth").into());
        }
        let (value, left) = visit(self, count)?;
        self.depth -= 1;
        if left != 0 {
            return Err(CodecError::Message(format!(
                "{left} unread container entries"
            )));
        }
        Ok(value)
    }
}

struct Access<'a, 'de> {
    de: &'a mut Decoder<'de>,
    left: usize,
}

impl<'de> SeqAccess<'de> for Access<'_, 'de> {
    type Error = CodecError;
    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, CodecError> {
        if self.left == 0 {
            return Ok(None);
        }
        self.left -= 1;
        seed.deserialize(&mut *self.de).map(Some)
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.left)
    }
}

impl<'de> MapAccess<'de> for Access<'_, 'de> {
    type Error = CodecError;
    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, CodecError> {
        if self.left == 0 {
            return Ok(None);
        }
        self.left -= 1;
        seed.deserialize(&mut *self.de).map(Some)
    }
    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, CodecError> {
        seed.deserialize(&mut *self.de)
    }
    fn size_hint(&self) -> Option<usize> {
        Some(self.left)
    }
}

/// The `{variant: content}` form of a non-unit enum variant.
struct Variant<'a, 'de> {
    de: &'a mut Decoder<'de>,
}

impl<'de> EnumAccess<'de> for Variant<'_, 'de> {
    type Error = CodecError;
    type Variant = Self;
    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self), CodecError> {
        let name = seed.deserialize(&mut *self.de)?;
        Ok((name, self))
    }
}

impl<'de> VariantAccess<'de> for Variant<'_, 'de> {
    type Error = CodecError;
    fn unit_variant(self) -> Result<(), CodecError> {
        de::Deserialize::deserialize(self.de)
    }
    fn newtype_variant_seed<T: DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<T::Value, CodecError> {
        seed.deserialize(self.de)
    }
    fn tuple_variant<V: Visitor<'de>>(self, _: usize, visitor: V) -> Result<V::Value, CodecError> {
        self.de.deserialize_any(visitor)
    }
    fn struct_variant<V: Visitor<'de>>(
        self,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, CodecError> {
        self.de.deserialize_any(visitor)
    }
}

impl<'de> Deserializer<'de> for &mut Decoder<'de> {
    type Error = CodecError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, CodecError> {
        match self.r.u8()? {
            NULL => visitor.visit_unit(),
            FALSE => visitor.visit_bool(false),
            TRUE => visitor.visit_bool(true),
            UINT => visitor.visit_u64(self.r.uvar()?),
            NINT => {
                let n = i64::try_from(self.r.uvar()?)
                    .map_err(|_| DecodeError::OutOfRange("negative integer"))?;
                visitor.visit_i64(-1 - n)
            }
            STR => visitor.visit_borrowed_str(self.r.str()?),
            BYTES => visitor.visit_borrowed_bytes(self.r.bytes()?),
            SEQ => self.container(|de, left| {
                let mut access = Access { de, left };
                let value = visitor.visit_seq(&mut access)?;
                Ok((value, access.left))
            }),
            MAP => self.container(|de, left| {
                let mut access = Access { de, left };
                let value = visitor.visit_map(&mut access)?;
                Ok((value, access.left))
            }),
            tag => Err(DecodeError::BadTag { what: "value", tag }.into()),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, CodecError> {
        if self.r.peek()? == NULL {
            self.r.u8()?;
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, CodecError> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, CodecError> {
        match self.r.u8()? {
            STR => visitor.visit_enum(self.r.str()?.into_deserializer()),
            MAP => {
                if self.r.uvar()? != 1 {
                    return Err(DecodeError::Inconsistent("enum map must have one entry").into());
                }
                visitor.visit_enum(Variant { de: self })
            }
            tag => Err(DecodeError::BadTag { what: "enum", tag }.into()),
        }
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
        unit unit_struct seq tuple tuple_struct map struct identifier ignored_any
    }
}
