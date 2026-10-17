//! Containers for the codec's serializer.

use super::ser::Encoder;
use super::CodecError;
use crate::bytes::varint;
use serde::ser::{
    Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant, SerializeTuple,
    SerializeTupleStruct, SerializeTupleVariant,
};

/// A sequence or map being written; its element count is backpatched when it closes, so the
/// encoding never trusts serde's length hint.
pub(crate) struct Compound<'a> {
    enc: &'a mut Encoder,
    at: usize,
    count: u64,
}

impl<'a> Compound<'a> {
    pub(super) const fn new(enc: &'a mut Encoder, at: usize) -> Self {
        Self { enc, at, count: 0 }
    }

    fn element<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), CodecError> {
        self.count += 1;
        value.serialize(&mut *self.enc)
    }

    fn field<T: Serialize + ?Sized>(&mut self, key: &str, value: &T) -> Result<(), CodecError> {
        self.count += 1;
        self.enc.str(key);
        value.serialize(&mut *self.enc)
    }

    fn close(self) {
        self.enc.w.insert_at(self.at, &varint(self.count));
    }
}

macro_rules! sequence_like {
    ($($trait:ident :: $method:ident),*) => {$(
        impl $trait for Compound<'_> {
            type Ok = ();
            type Error = CodecError;
            fn $method<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), CodecError> {
                self.element(value)
            }
            fn end(self) -> Result<(), CodecError> {
                self.close();
                Ok(())
            }
        }
    )*};
}

sequence_like!(
    SerializeSeq::serialize_element,
    SerializeTuple::serialize_element,
    SerializeTupleStruct::serialize_field,
    SerializeTupleVariant::serialize_field
);

macro_rules! struct_like {
    ($($trait:ident),*) => {$(
        impl $trait for Compound<'_> {
            type Ok = ();
            type Error = CodecError;
            fn serialize_field<T: Serialize + ?Sized>(
                &mut self,
                key: &'static str,
                value: &T,
            ) -> Result<(), CodecError> {
                self.field(key, value)
            }
            fn end(self) -> Result<(), CodecError> {
                self.close();
                Ok(())
            }
        }
    )*};
}

struct_like!(SerializeStruct, SerializeStructVariant);

impl SerializeMap for Compound<'_> {
    type Ok = ();
    type Error = CodecError;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), CodecError> {
        self.element(key)
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), CodecError> {
        value.serialize(&mut *self.enc)
    }
    fn end(self) -> Result<(), CodecError> {
        self.close();
        Ok(())
    }
}
