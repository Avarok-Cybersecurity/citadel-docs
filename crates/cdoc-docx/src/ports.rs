//! Images. The model names them by content id and the bytes live in the workspace's content
//! store, so a `.docx` is written through the shared [`ImagePort`] and read through a
//! [`MediaSink`] that keeps each embedded image and says what id the model should use.

use cdoc_model::ports::{ImageBytes, ImageError, ImageLibrary, ImagePort};
use cdoc_model::ContentId;

/// Where the reader puts the images it finds.
pub trait MediaSink {
    /// Keep an image and return the content id the model should name it by.
    fn store(&mut self, image: ImageBytes<'_>) -> Result<ContentId, ImageError>;
}

/// An in-memory store implementing both directions, for callers that move images across a
/// boundary in one piece (the WASM facade) and for tests. Stored images get sequential ids
/// `{prefix}{n}`; the prefix is the caller's choice. An image already held is not stored twice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryMedia {
    prefix: String,
    library: ImageLibrary,
    order: Vec<ContentId>,
}

impl MemoryMedia {
    /// An empty store whose minted ids start with `prefix`.
    #[must_use]
    pub fn with_prefix(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_owned(),
            library: ImageLibrary::empty(),
            order: Vec::new(),
        }
    }

    /// Add an image under an id the caller chose, replacing any image with that id.
    pub fn insert(&mut self, id: ContentId, mime: &str, bytes: Vec<u8>) {
        if !self.order.contains(&id) {
            self.order.push(id.clone());
        }
        self.library.add(id, mime, bytes);
    }

    /// The ids held, in insertion order.
    #[must_use]
    pub fn ids(&self) -> &[ContentId] {
        &self.order
    }
}

impl ImagePort for MemoryMedia {
    fn image(&self, id: &ContentId) -> Result<ImageBytes<'_>, ImageError> {
        self.library.image(id)
    }
}

impl MediaSink for MemoryMedia {
    fn store(&mut self, image: ImageBytes<'_>) -> Result<ContentId, ImageError> {
        let held = self
            .order
            .iter()
            .find(|id| self.library.image(id).is_ok_and(|held| held == image));
        if let Some(id) = held {
            return Ok(id.clone());
        }
        let id = ContentId::new(format!("{}{}", self.prefix, self.order.len() + 1));
        self.insert(id.clone(), image.mime, image.bytes.to_vec());
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(bytes: &[u8]) -> ImageBytes<'_> {
        ImageBytes {
            mime: "image/png",
            bytes,
        }
    }

    #[test]
    fn stored_images_get_sequential_prefixed_ids_and_duplicates_share_one() {
        let mut media = MemoryMedia::with_prefix("img-");
        let a = media.store(png(b"aaa")).expect("stores");
        let b = media.store(png(b"bbb")).expect("stores");
        let again = media.store(png(b"aaa")).expect("stores");
        assert_eq!(
            (a.as_str(), b.as_str(), again.as_str()),
            ("img-1", "img-2", "img-1")
        );
        assert_eq!(media.ids(), [a, b]);
    }

    #[test]
    fn the_same_bytes_under_another_type_are_another_image() {
        let mut media = MemoryMedia::with_prefix("i");
        let first = media.store(png(b"x")).expect("stores");
        let second = media
            .store(ImageBytes {
                mime: "image/gif",
                bytes: b"x",
            })
            .expect("stores");
        assert_ne!(first, second);
    }

    #[test]
    fn inserting_under_a_held_id_replaces_it_in_place() {
        let mut media = MemoryMedia::with_prefix("i");
        media.insert(ContentId::new("k"), "image/png", vec![1]);
        media.insert(ContentId::new("other"), "image/png", vec![2]);
        media.insert(ContentId::new("k"), "image/gif", vec![3]);
        assert_eq!(media.ids(), [ContentId::new("k"), ContentId::new("other")]);
        let held = media.image(&ContentId::new("k")).expect("held");
        assert_eq!((held.mime, held.bytes), ("image/gif", &[3u8][..]));
        assert!(media.image(&ContentId::new("absent")).is_err());
    }
}
