//! An added image over the boundary.
//!
//! ```text
//! added_image := "CDI" 0x01 content_id:str kind:u8 width:varint height:varint
//! kind        := 0 PNG | 1 JPEG
//! ```

use super::{Reader, Writer};
use crate::editor::AddedImage;
use cdoc_crdt::DecodeError;
use cdoc_model::image_probe::ImageKind;
use cdoc_model::ContentId;

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
