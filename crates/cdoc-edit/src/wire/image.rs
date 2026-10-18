//! An added image over the boundary.
//!
//! ```text
//! added_image := "CDI" 0x01 content_id:str kind:u8 width:varint height:varint
//! kind        := 0 PNG | 1 JPEG
//! image       := "CDB" 0x01 mime:str bytes:bytes
//! images      := "CDL" 0x01 count:varint (content_id:str mime:str byte_len:varint){count}
//! ```

use super::{Reader, Writer};
use crate::editor::AddedImage;
use cdoc_crdt::DecodeError;
use cdoc_model::image_probe::ImageKind;
use cdoc_model::{ContentId, ImageBytes, ImageLibrary};

const MAGIC: &[u8; 4] = b"CDI\x01";
const KINDS: [ImageKind; 2] = [ImageKind::Png, ImageKind::Jpeg];

/// The binary form of an added image.
#[must_use]
pub fn encode_added_image(a: &AddedImage) -> Vec<u8> {
    let mut w = Writer::new();
    w.raw(MAGIC);
    w.str(a.content_id.as_str());
    w.u8(match a.kind {
        ImageKind::Png => 0,
        ImageKind::Jpeg => 1,
    });
    w.uvar(u64::from(a.width));
    w.uvar(u64::from(a.height));
    w.finish()
}

/// Reads [`encode_added_image`]'s output.
pub fn decode_added_image(bytes: &[u8]) -> Result<AddedImage, DecodeError> {
    let mut r = Reader::new(bytes);
    if r.take(4)? != MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let content_id = ContentId::new(r.str()?);
    let tag = r.u8()?;
    let kind = *KINDS.get(usize::from(tag)).ok_or(DecodeError::BadTag {
        what: "image kind",
        tag,
    })?;
    let px = |r: &mut Reader<'_>| {
        u32::try_from(r.uvar()?).map_err(|_| DecodeError::OutOfRange("image size"))
    };
    let (width, height) = (px(&mut r)?, px(&mut r)?);
    r.finish()?;
    Ok(AddedImage {
        content_id,
        kind,
        width,
        height,
    })
}

const BYTES_MAGIC: &[u8; 4] = b"CDB\x01";
const LIST_MAGIC: &[u8; 4] = b"CDL\x01";

/// One held image's type and bytes, for the canvas painter.
#[must_use]
pub fn encode_image(image: &ImageBytes<'_>) -> Vec<u8> {
    let mut w = Writer::new();
    w.raw(BYTES_MAGIC);
    w.str(image.mime);
    w.bytes(image.bytes);
    w.finish()
}

/// Reads [`encode_image`]'s output: the MIME type and the bytes.
pub fn decode_image(bytes: &[u8]) -> Result<(String, Vec<u8>), DecodeError> {
    let mut r = Reader::new(bytes);
    if r.take(4)? != BYTES_MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let mime = r.str()?.to_owned();
    let data = r.bytes()?.to_vec();
    r.finish()?;
    Ok((mime, data))
}

/// Every held image's id, type and size, for warming a cache.
#[must_use]
pub fn encode_image_list(images: &ImageLibrary) -> Vec<u8> {
    let mut w = Writer::new();
    w.raw(LIST_MAGIC);
    let all: Vec<_> = images.iter().collect();
    w.usize(all.len());
    for (id, image) in all {
        w.str(id.as_str());
        w.str(image.mime);
        w.usize(image.bytes.len());
    }
    w.finish()
}

/// Reads [`encode_image_list`]'s output.
pub fn decode_image_list(bytes: &[u8]) -> Result<Vec<(ContentId, String, usize)>, DecodeError> {
    let mut r = Reader::new(bytes);
    if r.take(4)? != LIST_MAGIC {
        return Err(DecodeError::BadMagic);
    }
    let n = r.usize()?;
    let mut out = Vec::new();
    for _ in 0..n {
        out.push((ContentId::new(r.str()?), r.str()?.to_owned(), r.usize()?));
    }
    r.finish()?;
    Ok(out)
}
