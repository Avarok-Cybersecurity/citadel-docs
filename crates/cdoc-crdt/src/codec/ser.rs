//! The writing half of the codec: a `serde::Serializer` into a [`Writer`].

use super::compound::Compound;
use super::{CodecError, BYTES, FALSE, MAP, NINT, NULL, SEQ, STR, TRUE, UINT};
use crate::bytes::Writer;
use serde::ser::{Serialize, Serializer};

pub(crate) struct Encoder {
    pub(super) w: Writer,
}

impl Encoder {
    pub(crate) const fn new() -> Self {
        Self { w: Writer::new() }
    }

    pub(crate) fn finish(self) -> Vec<u8> {
        self.w.finish()
    }

    fn int(&mut self, v: i64) {
        if v >= 0 {
            self.w.u8(UINT);
            self.w.uvar(v.unsigned_abs());
        } else {
            self.w.u8(NINT);
            self.w.uvar((v + 1).unsigned_abs());
        }
    }

    pub(super) fn str(&mut self, s: &str) {
        self.w.u8(STR);
        self.w.str(s);
    }

    /// Opens a container whose element count is written when it closes.
    fn open(&mut self, tag: u8) -> Compound<'_> {
        self.w.u8(tag);
        let at = self.w.len();
        Compound::new(self, at)
    }

    /// The one-entry map `{variant: ...}` that wraps a non-unit enum variant.
    fn variant(&mut self, variant: &str) {
        self.w.u8(MAP);
        self.w.uvar(1);
        self.str(variant);
    }
}

impl<'a> Serializer for &'a mut Encoder {
    type Ok = ();
    type Error = CodecError;
    type SerializeSeq = Compound<'a>;
    type SerializeTuple = Compound<'a>;
    type SerializeTupleStruct = Compound<'a>;
    type SerializeTupleVariant = Compound<'a>;
    type SerializeMap = Compound<'a>;
    type SerializeStruct = Compound<'a>;
    type SerializeStructVariant = Compound<'a>;

    fn serialize_bool(self, v: bool) -> Result<(), CodecError> {
        self.w.u8(if v { TRUE } else { FALSE });
        Ok(())
    }
    fn serialize_i8(self, v: i8) -> Result<(), CodecError> {
        self.serialize_i64(v.into())
    }
    fn serialize_i16(self, v: i16) -> Result<(), CodecError> {
        self.serialize_i64(v.into())
    }
    fn serialize_i32(self, v: i32) -> Result<(), CodecError> {
        self.serialize_i64(v.into())
    }
    fn serialize_i64(self, v: i64) -> Result<(), CodecError> {
        self.int(v);
        Ok(())
    }
    fn serialize_u8(self, v: u8) -> Result<(), CodecError> {
        self.serialize_u64(v.into())
    }
    fn serialize_u16(self, v: u16) -> Result<(), CodecError> {
        self.serialize_u64(v.into())
    }
    fn serialize_u32(self, v: u32) -> Result<(), CodecError> {
        self.serialize_u64(v.into())
    }
    fn serialize_u64(self, v: u64) -> Result<(), CodecError> {
        self.w.u8(UINT);
        self.w.uvar(v);
        Ok(())
    }
    fn serialize_f32(self, _: f32) -> Result<(), CodecError> {
        Err(CodecError::Float)
    }
    fn serialize_f64(self, _: f64) -> Result<(), CodecError> {
        Err(CodecError::Float)
    }
    fn serialize_char(self, v: char) -> Result<(), CodecError> {
        self.str(v.encode_utf8(&mut [0; 4]));
        Ok(())
    }
    fn serialize_str(self, v: &str) -> Result<(), CodecError> {
        self.str(v);
        Ok(())
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<(), CodecError> {
        self.w.u8(BYTES);
        self.w.bytes(v);
        Ok(())
    }
    fn serialize_none(self) -> Result<(), CodecError> {
        self.serialize_unit()
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<(), CodecError> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), CodecError> {
        self.w.u8(NULL);
        Ok(())
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), CodecError> {
        self.serialize_unit()
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<(), CodecError> {
        self.str(variant);
        Ok(())
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<(), CodecError> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), CodecError> {
        self.variant(variant);
        value.serialize(self)
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Compound<'a>, CodecError> {
        Ok(self.open(SEQ))
    }
    fn serialize_tuple(self, _: usize) -> Result<Compound<'a>, CodecError> {
        Ok(self.open(SEQ))
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Compound<'a>, CodecError> {
        Ok(self.open(SEQ))
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Compound<'a>, CodecError> {
        self.variant(variant);
        Ok(self.open(SEQ))
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Compound<'a>, CodecError> {
        Ok(self.open(MAP))
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Compound<'a>, CodecError> {
        Ok(self.open(MAP))
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        _: usize,
    ) -> Result<Compound<'a>, CodecError> {
        self.variant(variant);
        Ok(self.open(MAP))
    }
}
