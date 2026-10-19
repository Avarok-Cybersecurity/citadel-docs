//! Images. The model names them by content id and the bytes live in the workspace's content
//! store, so a package is written through the shared [`ImagePort`] and read through a
//! [`MediaSink`] that keeps each embedded image and says what id the model should use.

use cdoc_model::ports::{ImageBytes, ImageError, ImageLibrary, ImagePort};
use cdoc_model::ContentId;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// What identifies an image's content: its type and the SHA-256 of its bytes.
type Key = (String, [u8; 32]);

fn key(mime: &str, bytes: &[u8]) -> Key {
    (mime.to_owned(), Sha256::digest(bytes).into())
}

/// Where the reader puts the images it finds.
pub trait MediaSink {
    /// Keep an image and return the content id the model should name it by.
    fn store(&mut self, image: ImageBytes<'_>) -> Result<ContentId, ImageError>;
}

/// An in-memory store implementing both directions, for callers that move images across a
/// boundary in one piece (the WASM facade) and for tests. Stored images get sequential ids
/// `{prefix}{n}`; the prefix is the caller's choice. An image already held is not stored twice:
/// it is found by its type and digest (bytes compared only on a match), so storing `n` images
/// costs `n` hashes, not a comparison with every image held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryMedia {
    prefix: String,
    library: ImageLibrary,
    order: Vec<ContentId>,
    keys: HashMap<ContentId, Key>,
    by_key: HashMap<Key, ContentId>,
}

impl MemoryMedia {
    /// An empty store whose minted ids start with `prefix`.
    #[must_use]
    pub fn with_prefix(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_owned(),
            library: ImageLibrary::empty(),
            order: Vec::new(),
            keys: HashMap::new(),
            by_key: HashMap::new(),
        }
    }

    /// Add an image under an id the caller chose, replacing any image with that id.
    pub fn insert(&mut self, id: ContentId, mime: &str, bytes: Vec<u8>) {
        let k = key(mime, &bytes);
        match self.keys.insert(id.clone(), k.clone()) {
            Some(old) => {
                if self.by_key.get(&old) == Some(&id) {
                    self.by_key.remove(&old);
                }
            }
            None => self.order.push(id.clone()),
        }
        self.by_key.entry(k).or_insert_with(|| id.clone());
        self.library.add(id, mime, bytes);
    }

    /// The ids held, in insertion order.
    #[must_use]
    pub fn ids(&self) -> &[ContentId] {
        &self.order
    }

    /// Every image as `(id, mime, bytes)`, in insertion order, moved out of the store.
    #[must_use]
    pub fn into_images(mut self) -> Vec<(ContentId, String, Vec<u8>)> {
        let library = &mut self.library;
        self.order
            .into_iter()
            .filter_map(|id| library.remove(&id).map(|(mime, bytes)| (id, mime, bytes)))
            .collect()
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
            .by_key
            .get(&key(image.mime, image.bytes))
            .filter(|id| self.library.image(id).is_ok_and(|held| held == image));
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
    fn a_replaced_image_is_found_by_its_new_content_only() {
        let mut media = MemoryMedia::with_prefix("i");
        media.insert(ContentId::new("k"), "image/png", vec![1]);
        media.insert(ContentId::new("k"), "image/png", vec![2]);
        assert_eq!(media.store(png(&[2])).expect("stores"), ContentId::new("k"));
        assert_ne!(media.store(png(&[1])).expect("stores"), ContentId::new("k"));
    }

    /// Ten thousand images alike but for their last byte: storing them is linear. Compared
    /// byte for byte with every image held, as the store once did, they take minutes.
    #[test]
    fn many_near_identical_images_store_in_linear_time() {
        let mut media = MemoryMedia::with_prefix("i");
        let mut file = vec![7u8; 2048];
        let start = std::time::Instant::now();
        for n in 0..10_000u32 {
            file[2044..].copy_from_slice(&n.to_le_bytes());
            media.store(png(&file)).expect("stores");
        }
        let took = start.elapsed();
        assert_eq!(media.ids().len(), 10_000);
        assert!(took < std::time::Duration::from_secs(5), "took {took:?}");
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
