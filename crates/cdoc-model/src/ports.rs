//! Ports more than one crate takes from the world. Image bytes live in the workspace's content
//! store, and the model names images by content id; whoever draws or exports an image asks an
//! [`ImagePort`] for its bytes.

use crate::ids::ContentId;
use std::collections::BTreeMap;

/// An image's bytes as stored, with the MIME type its uploader declared. Decoding (and checking
/// that the bytes are what the type says) stays in the crate that uses them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageBytes<'a> {
    /// The MIME type, e.g. `image/png`.
    pub mime: &'a str,
    /// The file.
    pub bytes: &'a [u8],
}

/// An image the port could not provide.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("no image with content id {0:?}")]
pub struct ImageError(pub String);

/// Where image bytes come from: the workspace's content store, through the caller.
pub trait ImagePort {
    /// The bytes of the image with this content id.
    fn image(&self, id: &ContentId) -> Result<ImageBytes<'_>, ImageError>;
}

/// Images held in memory, by content id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageLibrary {
    images: BTreeMap<ContentId, (String, Vec<u8>)>,
}

impl ImageLibrary {
    /// No images.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            images: BTreeMap::new(),
        }
    }

    /// Registers an image.
    pub fn add(&mut self, id: ContentId, mime: &str, bytes: Vec<u8>) {
        self.images.insert(id, (mime.to_owned(), bytes));
    }
}

impl ImagePort for ImageLibrary {
    fn image(&self, id: &ContentId) -> Result<ImageBytes<'_>, ImageError> {
        self.images
            .get(id)
            .map(|(mime, bytes)| ImageBytes { mime, bytes })
            .ok_or_else(|| ImageError(id.as_str().to_owned()))
    }
}
