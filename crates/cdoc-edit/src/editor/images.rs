//! Images the user adds: checked by their header, named by their content, and held for
//! whoever draws or exports them (an [`ImagePort`](cdoc_model::ImagePort)). The id is
//! `sha256-` and the hex digest of the bytes, so every peer and the content store derive the
//! same id from the same file and can check the bytes they fetch against it.

use super::Editor;
use crate::error::EditError;
use cdoc_model::image_probe::{probe, ImageKind};
use cdoc_model::{ContentId, ImageLibrary};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;

/// An image the editor now holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedImage {
    /// The id to insert it by (`Command::InsertImage`) and to store its bytes under.
    pub content_id: ContentId,
    /// Its type, from the file's signature.
    pub kind: ImageKind,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

fn content_id(bytes: &[u8]) -> ContentId {
    let mut id = String::from("sha256-");
    for b in Sha256::digest(bytes) {
        write!(id, "{b:02x}").expect("writing to a string");
    }
    ContentId::new(id)
}

impl Editor<'_> {
    /// Holds an image file of at most `max_bytes` bytes and returns its id and pixel size.
    /// PNG and JPEG are accepted, recognised by their signature; anything else is refused by
    /// name. The display size is the caller's (`Command::InsertImage`): pixels carry no
    /// physical size the editor could trust.
    pub fn add_image(&mut self, bytes: Vec<u8>, max_bytes: usize) -> Result<AddedImage, EditError> {
        if bytes.len() > max_bytes {
            return Err(EditError::ImageTooLarge {
                size: bytes.len(),
                limit: max_bytes,
            });
        }
        let header = probe(&bytes)?;
        let id = content_id(&bytes);
        self.images.add(id.clone(), header.kind.mime(), bytes);
        Ok(AddedImage {
            content_id: id,
            kind: header.kind,
            width: header.width,
            height: header.height,
        })
    }

    /// The images added in this session, by content id.
    #[must_use]
    pub const fn images(&self) -> &ImageLibrary {
        &self.images
    }
}
