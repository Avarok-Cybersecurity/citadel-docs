//! What the backends take from the world: image bytes and the time. Fonts come through
//! `cdoc_layout::FontPort`, the same port the layout measured with.

use cdoc_model::ContentId;
use std::collections::BTreeMap;

/// An image's bytes as stored, with the MIME type its uploader declared. Decoding (and checking
/// that the bytes are what the type says) stays in this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageBytes<'a> {
    /// `image/jpeg` or `image/png`.
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

/// The time, for the dates a PDF records about itself. Only the caller has a clock.
pub trait ClockPort {
    /// Now, as RFC 3339 (`2026-10-04T12:00:00Z`).
    fn now_rfc3339(&self) -> String;
}

/// A clock that always says the same time: what the WASM facade builds from the time the
/// browser passed in, and what tests use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedClock(pub String);

impl ClockPort for FixedClock {
    fn now_rfc3339(&self) -> String {
        self.0.clone()
    }
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
